//! Stage 16.77 MUV-2: `impl ArithmeticEmitter for TextEmitter`.
//!
//! Extracted from `text/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use crate::codegen::emitter::*;
use crate::mir::place::{BinOp, UnOp};
use crate::mir::ty::ConstVal;

use super::TextEmitter;
use super::{binop_to_llvm_str, emit_type_to_llvm_str};

impl ArithmeticEmitter for TextEmitter {
    fn emit_const(&mut self, val: &ConstVal) -> EmitValue {
        match val {
            ConstVal::Int(n) => format!("{}", n),
            ConstVal::Uint(n) => format!("{}", n),
            ConstVal::Bool(b) => format!("{}", if *b { 1 } else { 0 }),
            ConstVal::Float(bits) => {
                let f = f64::from_bits(*bits);
                if f == 0.0 {
                    "0.000000e+00".to_string()
                } else if f == 1.0 {
                    "1.000000e+00".to_string()
                } else {
                    format!("{:e}", f)
                }
            }
            ConstVal::Char(c) => format!("{}", *c as u32),
            // Stage 3.27: emit a module-level global and return its name.
            // The caller will treat the value as a `i8*` (pointer to the
            // first byte of the global). Full &str fat-pointer (ptr+len)
            // representation is deferred — Stage 3.27 just gives the ptr.
            ConstVal::Str(sym) => {
                // The symbol's bytes are looked up by the codegen translation
                // layer (which has the interner) and passed to
                // `emit_string_global`. By the time we get here, the value
                // has already been turned into a global name and stored in
                // `string_globals` under the symbol's key. We can't call
                // `emit_string_global` from here because we don't have the
                // bytes — the codegen layer intercepts Str before calling
                // emit_const.
                //
                // Fallback: if emit_const is called directly with Str,
                // emit a placeholder zero pointer.
                let _ = sym;
                "0".to_string()
            }
            ConstVal::Unevaluated => "0".to_string(),
        }
    }

    fn emit_binop(
        &mut self,
        op: BinOp,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        let r = self.fresh();
        let opc = binop_to_llvm_str(op, ty);
        self.line(&format!("  %v{} = {} {}, {}", r, opc, lhs, rhs));
        format!("%v{}", r)
    }

    fn emit_unop(&mut self, op: UnOp, ty: &EmitType, operand: &EmitValue) -> EmitValue {
        let r = self.fresh();
        match op {
            UnOp::Neg => {
                let ty_str = emit_type_to_llvm_str(ty);
                if *ty == EmitType::F64 || *ty == EmitType::F32 {
                    self.line(&format!("  %v{} = fneg {} {}", r, ty_str, operand));
                } else {
                    self.line(&format!("  %v{} = sub {} 0, {}", r, ty_str, operand));
                }
            }
            UnOp::Not => {
                let ty_str = emit_type_to_llvm_str(ty);
                self.line(&format!("  %v{} = xor {} {}, -1", r, ty_str, operand));
            }
        }
        format!("%v{}", r)
    }

    fn emit_icmp(
        &mut self,
        op: &str,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!(
            "  %v{} = icmp {} {} {}, {}",
            r, op, ty_str, lhs, rhs
        ));
        format!("%v{}", r)
    }

    fn emit_fcmp(
        &mut self,
        op: &str,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!(
            "  %v{} = fcmp {} {} {}, {}",
            r, op, ty_str, lhs, rhs
        ));
        format!("%v{}", r)
    }

    /// Stage 3.49 (L13 closure): bitwise AND for fat-pointer eq comparison.
    fn emit_and(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!("  %v{} = and {} {}, {}", r, ty_str, lhs, rhs));
        format!("%v{}", r)
    }

    /// Stage 3.49 (L13 closure): bitwise OR for fat-pointer ne comparison.
    fn emit_or(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!("  %v{} = or {} {}, {}", r, ty_str, lhs, rhs));
        format!("%v{}", r)
    }

    fn emit_zext(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let src_str = emit_type_to_llvm_str(src);
        let dst_str = emit_type_to_llvm_str(dst);
        self.line(&format!(
            "  %v{} = zext {} {} to {}",
            r, src_str, val, dst_str
        ));
        format!("%v{}", r)
    }

    fn emit_cast(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let src_str = emit_type_to_llvm_str(src);
        let dst_str = emit_type_to_llvm_str(dst);
        // Stage 14.65: Generalize integer-to-integer casts.
        //
        // Previously, only specific pairs were handled (I32→I64 sext, I1→I32
        // zext, I64→I32/I32→I1 trunc). Other integer pairs (e.g., I32→I8 for
        // `c as char`) fell through to "bitcast", which is invalid for integers
        // of different widths.
        //
        // Fix: for ANY integer-to-integer cast, choose the right op based on
        // width comparison:
        //   - src == dst width: bitcast (no-op, same size)
        //   - src < dst: zext (zero-extend, for unsigned) or sext (sign-extend,
        //     for signed — Landin defaults to signed)
        //   - src > dst: trunc
        //
        // Per §1.0 原则 6 "通用 > 特例": one rule for all integer pairs.
        let is_int = |t: &EmitType| {
            matches!(
                t,
                EmitType::I1
                    | EmitType::I8
                    | EmitType::I16
                    | EmitType::I32
                    | EmitType::I64
                    | EmitType::I128
            )
        };
        let int_width = |t: &EmitType| -> u32 {
            match t {
                EmitType::I1 => 1,
                EmitType::I8 => 8,
                EmitType::I16 => 16,
                EmitType::I32 => 32,
                EmitType::I64 => 64,
                EmitType::I128 => 128,
                _ => 0,
            }
        };
        let op = match (src, dst) {
            (a, b) if a == b => return val.clone(),
            (a, b) if is_int(a) && is_int(b) => {
                let sw = int_width(a);
                let dw = int_width(b);
                if sw < dw {
                    "sext" // sign-extend (Landin integers default to signed)
                } else if sw > dw {
                    "trunc"
                } else {
                    "bitcast" // same width (rare for integers, but valid)
                }
            }
            (EmitType::I32, EmitType::F64) | (EmitType::I64, EmitType::F64) => "sitofp",
            (EmitType::I32, EmitType::F32) | (EmitType::I64, EmitType::F32) => "sitofp",
            (EmitType::I8, EmitType::F64) | (EmitType::I8, EmitType::F32) => "sitofp",
            (EmitType::I16, EmitType::F64) | (EmitType::I16, EmitType::F32) => "sitofp",
            (EmitType::F64, EmitType::I32) | (EmitType::F64, EmitType::I64) => "fptosi",
            (EmitType::F32, EmitType::I32) | (EmitType::F32, EmitType::I64) => "fptosi",
            (EmitType::F64, EmitType::I8) | (EmitType::F32, EmitType::I8) => "fptosi",
            (EmitType::F64, EmitType::I16) | (EmitType::F32, EmitType::I16) => "fptosi",
            (EmitType::F64, EmitType::F32) => "fptrunc",
            (EmitType::F32, EmitType::F64) => "fpext",
            // Stage 18.326 B1 (P1 soundness fix): int → ptr cast must use
            // `inttoptr`, NOT `bitcast`. `bitcast i32 0 to ptr` is invalid
            // LLVM IR that causes segfaults (LLVM may fold incorrectly).
            // Per LLVM Language Reference: int→ptr requires `inttoptr`.
            // Per Rust design: rustc_codegen_llvm uses `inttoptr` for int→ptr.
            // Per §2.2 (根因思维) + §1.0 原則 6 (通解>特解): one rule for all int→ptr.
            (a, EmitType::OpaquePtr) | (a, EmitType::Ptr(_)) if is_int(a) => "inttoptr",
            // Stage 18.326: ptr → int cast must use `ptrtoint`.
            (EmitType::OpaquePtr, a) | (EmitType::Ptr(_), a) if is_int(a) => "ptrtoint",
            _ => "bitcast",
        };
        self.line(&format!(
            "  %v{} = {} {} {} to {}",
            r, op, src_str, val, dst_str
        ));
        format!("%v{}", r)
    }

    /// Stage 18.205: Emit a null pointer constant for the text backend.
    ///
    /// Stage 18.326 B6 (P1 soundness fix): Return `"null"` (NOT `"ptr null"`).
    ///
    /// **Design boundary** (per Rust rustc_codegen_llvm):
    /// - `emit_null_ptr` returns a **value** (`null`), NOT a typed value.
    /// - Callers add the type prefix: `store ptr null`, `insertvalue ..., ptr null, ...`.
    /// - This matches the `format!("{} {}", ty_str, val)` pattern in `emit_store`,
    ///   `emit_call`, `emit_select`, etc.
    ///
    /// Previously returned `"ptr null"` which caused `store ptr ptr null`
    /// (double type prefix → invalid IR → segfault). Per §2.2 + §12.
    fn emit_null_ptr(&mut self) -> EmitValue {
        // Return value only — callers add "ptr" prefix via format!("{} {}", ty, val).
        "null".to_string()
    }

    /// Stage 14.12 (GAP-18): TextEmitter select instruction.
    /// Emits: `%vN = select i1 %cond, <ty> %true_val, <ty> %false_val`
    fn emit_select(
        &mut self,
        ty: &EmitType,
        cond: &EmitValue,
        true_val: &EmitValue,
        false_val: &EmitValue,
    ) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!(
            "  %v{} = select i1 {}, {} {}, {} {}",
            r, cond, ty_str, true_val, ty_str, false_val
        ));
        format!("%v{}", r)
    }

    fn emit_checked_binop(
        &mut self,
        op: BinOp,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        // Stage 3.24: emit `llvm.{sadd,ssub,smul}.with.overflow.{i32,i64}`.
        // Returns `{ T, i1 }` — caller extracts index 1 for the overflow flag.
        let elem_str = emit_type_to_llvm_str(ty);
        let intrinsic = match (op, ty) {
            (BinOp::Add, EmitType::I8) => "llvm.sadd.with.overflow.i8",
            (BinOp::Add, EmitType::I16) => "llvm.sadd.with.overflow.i16",
            (BinOp::Add, EmitType::I32) => "llvm.sadd.with.overflow.i32",
            (BinOp::Add, EmitType::I64) => "llvm.sadd.with.overflow.i64",
            (BinOp::Add, EmitType::I128) => "llvm.sadd.with.overflow.i128",
            (BinOp::Sub, EmitType::I8) => "llvm.ssub.with.overflow.i8",
            (BinOp::Sub, EmitType::I16) => "llvm.ssub.with.overflow.i16",
            (BinOp::Sub, EmitType::I32) => "llvm.ssub.with.overflow.i32",
            (BinOp::Sub, EmitType::I64) => "llvm.ssub.with.overflow.i64",
            (BinOp::Sub, EmitType::I128) => "llvm.ssub.with.overflow.i128",
            (BinOp::Mul, EmitType::I8) => "llvm.smul.with.overflow.i8",
            (BinOp::Mul, EmitType::I16) => "llvm.smul.with.overflow.i16",
            (BinOp::Mul, EmitType::I32) => "llvm.smul.with.overflow.i32",
            (BinOp::Mul, EmitType::I64) => "llvm.smul.with.overflow.i64",
            (BinOp::Mul, EmitType::I128) => "llvm.smul.with.overflow.i128",
            // Unsupported op or type — fall back to "no overflow".
            // Synthesize `{ T, i1 } undef` with the overflow flag zeroed.
            _ => {
                let r = self.fresh();
                let agg_str = format!("{{ {}, i1 }}", elem_str);
                self.line(&format!(
                    "  %v{} = insertvalue {} undef, {} 0, 1",
                    r, agg_str, elem_str
                ));
                return format!("%v{}", r);
            }
        };
        let r = self.fresh();
        let agg_str = format!("{{ {}, i1 }}", elem_str);
        self.line(&format!(
            "  %v{} = call {} @{}({} {}, {} {})",
            r, agg_str, intrinsic, elem_str, lhs, elem_str, rhs
        ));
        format!("%v{}", r)
    }

    /// Stage 18.287 (TD-NEGOVERFLOW-I32 fix): Emit a typed integer constant.
    ///
    /// Text emitter: just emit the value with the type string. This is used
    /// by the text backend for debugging/testing — the LLVM backend is the
    /// primary path for actual codegen.
    fn emit_const_typed(&mut self, val: i64, ty: &EmitType) -> EmitValue {
        let ty_str = match ty {
            EmitType::I1 => "i1",
            EmitType::I8 => "i8",
            EmitType::I16 => "i16",
            EmitType::I32 => "i32",
            EmitType::I64 => "i64",
            EmitType::I128 => "i128",
            EmitType::F32 => "f32",
            EmitType::F64 => "f64",
            _ => "i64",
        };
        format!("{} {}", ty_str, val)
    }
}
