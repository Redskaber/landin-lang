//! Stage 16.77 MUV-1: `impl ArithmeticEmitter for LLVMSysEmitter`.
//!
//! Extracted from `llvm/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use crate::codegen::emitter::ArithmeticEmitter;
use crate::codegen::emitter::*;
use crate::mir::place::{BinOp, UnOp};
use crate::mir::ty::ConstVal;
use llvm_sys::core::*;
use llvm_sys::prelude::*;
use std::ffi::CString;

use super::helpers::*;
use super::LLVMSysEmitter;

impl ArithmeticEmitter for LLVMSysEmitter {
    fn emit_const(&mut self, val: &ConstVal) -> EmitValue {
        unsafe {
            let v = match val {
                ConstVal::Int(n) => {
                    let ty = LLVMInt32TypeInContext(self.ctx);
                    LLVMConstInt(ty, *n as u64, 1)
                }
                ConstVal::Uint(n) => {
                    let ty = LLVMInt32TypeInContext(self.ctx);
                    LLVMConstInt(ty, *n as u64, 0)
                }
                ConstVal::Bool(b) => {
                    let ty = LLVMInt1TypeInContext(self.ctx);
                    LLVMConstInt(ty, if *b { 1 } else { 0 }, 0)
                }
                ConstVal::Char(c) => {
                    let ty = LLVMInt32TypeInContext(self.ctx);
                    LLVMConstInt(ty, *c as u64, 0)
                }
                ConstVal::Float(bits) => {
                    let ty = LLVMDoubleTypeInContext(self.ctx);
                    LLVMConstReal(ty, f64::from_bits(*bits))
                }
                ConstVal::Str(_) => {
                    // Stage 3.27 (TextEmitter) intercepts Str before reaching
                    // emit_const. Here we just return a null pointer.
                    LLVMConstNull(LLVMPointerTypeInContext(self.ctx, 0))
                }
                ConstVal::Unevaluated => {
                    let ty = LLVMInt32TypeInContext(self.ctx);
                    LLVMConstInt(ty, 0, 0)
                }
            };
            // Constants don't need a unique SSA name — return a synthetic one.
            self.fresh_named(v)
        }
    }

    fn emit_binop(
        &mut self,
        op: BinOp,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        let lhs_v = self.lookup(lhs);
        let rhs_v = self.lookup(rhs);
        unsafe {
            let v = match op {
                BinOp::Add => {
                    if is_float(ty) {
                        LLVMBuildFAdd(self.builder, lhs_v, rhs_v, cstr("add"))
                    } else {
                        LLVMBuildAdd(self.builder, lhs_v, rhs_v, cstr("add"))
                    }
                }
                BinOp::Sub => {
                    if is_float(ty) {
                        LLVMBuildFSub(self.builder, lhs_v, rhs_v, cstr("sub"))
                    } else {
                        LLVMBuildSub(self.builder, lhs_v, rhs_v, cstr("sub"))
                    }
                }
                BinOp::Mul => {
                    if is_float(ty) {
                        LLVMBuildFMul(self.builder, lhs_v, rhs_v, cstr("mul"))
                    } else {
                        LLVMBuildMul(self.builder, lhs_v, rhs_v, cstr("mul"))
                    }
                }
                BinOp::Div => {
                    if is_float(ty) {
                        LLVMBuildFDiv(self.builder, lhs_v, rhs_v, cstr("div"))
                    } else {
                        LLVMBuildSDiv(self.builder, lhs_v, rhs_v, cstr("div"))
                    }
                }
                BinOp::Rem => {
                    if is_float(ty) {
                        LLVMBuildFRem(self.builder, lhs_v, rhs_v, cstr("rem"))
                    } else {
                        LLVMBuildSRem(self.builder, lhs_v, rhs_v, cstr("rem"))
                    }
                }
                BinOp::BitAnd => LLVMBuildAnd(self.builder, lhs_v, rhs_v, cstr("and")),
                BinOp::BitOr => LLVMBuildOr(self.builder, lhs_v, rhs_v, cstr("or")),
                BinOp::BitXor => LLVMBuildXor(self.builder, lhs_v, rhs_v, cstr("xor")),
                BinOp::Shl => LLVMBuildShl(self.builder, lhs_v, rhs_v, cstr("shl")),
                BinOp::Shr => LLVMBuildAShr(self.builder, lhs_v, rhs_v, cstr("shr")),
                BinOp::Eq => LLVMBuildICmp(
                    self.builder,
                    llvm_sys::LLVMIntPredicate::LLVMIntEQ,
                    lhs_v,
                    rhs_v,
                    cstr("eq"),
                ),
                BinOp::Ne => LLVMBuildICmp(
                    self.builder,
                    llvm_sys::LLVMIntPredicate::LLVMIntNE,
                    lhs_v,
                    rhs_v,
                    cstr("ne"),
                ),
                BinOp::Lt => LLVMBuildICmp(
                    self.builder,
                    llvm_sys::LLVMIntPredicate::LLVMIntSLT,
                    lhs_v,
                    rhs_v,
                    cstr("lt"),
                ),
                BinOp::Le => LLVMBuildICmp(
                    self.builder,
                    llvm_sys::LLVMIntPredicate::LLVMIntSLE,
                    lhs_v,
                    rhs_v,
                    cstr("le"),
                ),
                BinOp::Ge => LLVMBuildICmp(
                    self.builder,
                    llvm_sys::LLVMIntPredicate::LLVMIntSGE,
                    lhs_v,
                    rhs_v,
                    cstr("ge"),
                ),
                BinOp::Gt => LLVMBuildICmp(
                    self.builder,
                    llvm_sys::LLVMIntPredicate::LLVMIntSGT,
                    lhs_v,
                    rhs_v,
                    cstr("gt"),
                ),
            };
            self.fresh_named(v)
        }
    }

    fn emit_unop(&mut self, op: UnOp, ty: &EmitType, operand: &EmitValue) -> EmitValue {
        let v = self.lookup(operand);
        unsafe {
            let res = match op {
                UnOp::Neg => {
                    if is_float(ty) {
                        LLVMBuildFNeg(self.builder, v, cstr("neg"))
                    } else {
                        LLVMBuildNeg(self.builder, v, cstr("neg"))
                    }
                }
                UnOp::Not => LLVMBuildNot(self.builder, v, cstr("not")),
            };
            self.fresh_named(res)
        }
    }

    fn emit_icmp(
        &mut self,
        op: &str,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        let _ = ty;
        let pred = parse_int_predicate(op);
        let lhs_v = self.lookup(lhs);
        let rhs_v = self.lookup(rhs);
        unsafe {
            let name_c = CString::new("icmp").unwrap();
            let v = LLVMBuildICmp(self.builder, pred, lhs_v, rhs_v, name_c.as_ptr());
            self.fresh_named(v)
        }
    }

    fn emit_fcmp(
        &mut self,
        op: &str,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        let _ = ty;
        let pred = parse_real_predicate(op);
        let lhs_v = self.lookup(lhs);
        let rhs_v = self.lookup(rhs);
        unsafe {
            let name_c = CString::new("fcmp").unwrap();
            let v = LLVMBuildFCmp(self.builder, pred, lhs_v, rhs_v, name_c.as_ptr());
            self.fresh_named(v)
        }
    }

    fn emit_and(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue {
        let _ = ty;
        unsafe {
            let lhs_v = self.lookup(lhs);
            let rhs_v = self.lookup(rhs);
            let name_c = CString::new("and").unwrap();
            let v = LLVMBuildAnd(self.builder, lhs_v, rhs_v, name_c.as_ptr());
            self.fresh_named(v)
        }
    }

    fn emit_or(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue {
        let _ = ty;
        unsafe {
            let lhs_v = self.lookup(lhs);
            let rhs_v = self.lookup(rhs);
            let name_c = CString::new("or").unwrap();
            let v = LLVMBuildOr(self.builder, lhs_v, rhs_v, name_c.as_ptr());
            self.fresh_named(v)
        }
    }

    fn emit_zext(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue {
        let _ = src;
        unsafe {
            let v = self.lookup(val);
            let dst_ty = self.llvm_type(dst);
            let name_c = CString::new("zext").unwrap();
            let r = LLVMBuildZExt(self.builder, v, dst_ty, name_c.as_ptr());
            self.fresh_named(r)
        }
    }

    fn emit_cast(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue {
        // Same-typecast short-circuit (mirrors TextEmitter behaviour).
        if src == dst {
            return val.clone();
        }
        unsafe {
            let v = self.lookup(val);
            let dst_ty = self.llvm_type(dst);
            let name_c = CString::new("cast").unwrap();
            // Stage 14.65: Generalize integer-to-integer casts.
            //
            // Previously, `emit_cast` only handled specific pairs:
            // (I32, I64) → SExt, (I1, I32) → ZExt, (I64, I32)/(I32, I1) → Trunc.
            // All other integer pairs (e.g., I32 → I8 for `c as char`, I8 → I32
            // for `char as i32`) fell through to `LLVMBuildBitCast`, which is
            // INVALID for integers of different widths — produces
            // "Invalid bitcast" LLVM verification errors.
            //
            // Fix: for ANY integer-to-integer cast, use `LLVMBuildIntCast2`
            // with `is_signed=1` (Landin integers default to signed). This
            // handles zext (wider), sext (wider, signed), and trunc (narrower)
            // automatically based on source/destination widths.
            //
            // Per §1.0 原则 6 "通用 > 特例": one rule for all integer pairs
            // instead of enumerating each combination.
            let src_kind = LLVMGetTypeKind(self.llvm_type(src));
            let dst_kind = LLVMGetTypeKind(dst_ty);
            let r = if src_kind == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
                && dst_kind == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
            {
                // Integer-to-integer: use IntCast2 (handles zext/sext/trunc).
                // Sign=1 means signed (SExt for widening, Trunc for narrowing).
                LLVMBuildIntCast2(self.builder, v, dst_ty, 1, name_c.as_ptr())
            } else {
                match (src, dst) {
                    (EmitType::I32, EmitType::F64)
                    | (EmitType::I64, EmitType::F64)
                    | (EmitType::I32, EmitType::F32)
                    | (EmitType::I64, EmitType::F32)
                    | (EmitType::I8, EmitType::F64)
                    | (EmitType::I8, EmitType::F32)
                    | (EmitType::I16, EmitType::F64)
                    | (EmitType::I16, EmitType::F32) => {
                        LLVMBuildSIToFP(self.builder, v, dst_ty, name_c.as_ptr())
                    }
                    (EmitType::F64, EmitType::I32)
                    | (EmitType::F64, EmitType::I64)
                    | (EmitType::F32, EmitType::I32)
                    | (EmitType::F32, EmitType::I64)
                    | (EmitType::F64, EmitType::I8)
                    | (EmitType::F32, EmitType::I8)
                    | (EmitType::F64, EmitType::I16)
                    | (EmitType::F32, EmitType::I16) => {
                        LLVMBuildFPToSI(self.builder, v, dst_ty, name_c.as_ptr())
                    }
                    (EmitType::F64, EmitType::F32) => {
                        LLVMBuildFPTrunc(self.builder, v, dst_ty, name_c.as_ptr())
                    }
                    (EmitType::F32, EmitType::F64) => {
                        LLVMBuildFPExt(self.builder, v, dst_ty, name_c.as_ptr())
                    }
                    _ => LLVMBuildBitCast(self.builder, v, dst_ty, name_c.as_ptr()),
                }
            };
            self.fresh_named(r)
        }
    }

    /// Stage 14.12 (GAP-18): LLVMSysEmitter select instruction.
    /// Uses LLVMBuildSelect to emit a `select` instruction that chooses
    /// between two values based on a boolean condition.
    fn emit_select(
        &mut self,
        ty: &EmitType,
        cond: &EmitValue,
        true_val: &EmitValue,
        false_val: &EmitValue,
    ) -> EmitValue {
        unsafe {
            let cond_v = self.lookup(cond);
            let true_v = self.lookup(true_val);
            let false_v = self.lookup(false_val);
            let _ = ty; // LLVM type is inferred from the values
            let name_c = CString::new("select").unwrap();
            let r = LLVMBuildSelect(self.builder, cond_v, true_v, false_v, name_c.as_ptr());
            self.fresh_named(r)
        }
    }

    fn emit_checked_binop(
        &mut self,
        op: BinOp,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        // Stage 14.103 (SH-5 fix): Implement real checked binop using LLVM
        // intrinsics `llvm.{sadd,ssub,smul}.with.overflow.{i8,i16,i32,i64,i128}`.
        //
        // Previously this was a stub that always returned overflow=0, silently
        // disabling overflow detection on the --emit-obj/--run path.
        //
        // Per §1.0 原则 5 "报错 > 静默": overflow checks must actually work.
        // Per §1.0 原则 6 "通用 > 特例": one intrinsic-name function handles
        // all op/type combinations.
        unsafe {
            let elem_ty = self.llvm_type(ty);
            let i1_ty = LLVMInt1TypeInContext(self.ctx);
            let fields = [elem_ty, i1_ty];
            let agg_ty =
                LLVMStructTypeInContext(self.ctx, fields.as_ptr() as *mut LLVMTypeRef, 2, 0);

            // Determine the intrinsic name based on op + type.
            let intrinsic_name: Option<String> = match (op, ty) {
                (BinOp::Add, EmitType::I8) => Some("llvm.sadd.with.overflow.i8".to_string()),
                (BinOp::Add, EmitType::I16) => Some("llvm.sadd.with.overflow.i16".to_string()),
                (BinOp::Add, EmitType::I32) => Some("llvm.sadd.with.overflow.i32".to_string()),
                (BinOp::Add, EmitType::I64) => Some("llvm.sadd.with.overflow.i64".to_string()),
                (BinOp::Add, EmitType::I128) => Some("llvm.sadd.with.overflow.i128".to_string()),
                (BinOp::Sub, EmitType::I8) => Some("llvm.ssub.with.overflow.i8".to_string()),
                (BinOp::Sub, EmitType::I16) => Some("llvm.ssub.with.overflow.i16".to_string()),
                (BinOp::Sub, EmitType::I32) => Some("llvm.ssub.with.overflow.i32".to_string()),
                (BinOp::Sub, EmitType::I64) => Some("llvm.ssub.with.overflow.i64".to_string()),
                (BinOp::Sub, EmitType::I128) => Some("llvm.ssub.with.overflow.i128".to_string()),
                (BinOp::Mul, EmitType::I8) => Some("llvm.smul.with.overflow.i8".to_string()),
                (BinOp::Mul, EmitType::I16) => Some("llvm.smul.with.overflow.i16".to_string()),
                (BinOp::Mul, EmitType::I32) => Some("llvm.smul.with.overflow.i32".to_string()),
                (BinOp::Mul, EmitType::I64) => Some("llvm.smul.with.overflow.i64".to_string()),
                (BinOp::Mul, EmitType::I128) => Some("llvm.smul.with.overflow.i128".to_string()),
                _ => None,
            };

            if let Some(name) = intrinsic_name {
                // Declare the intrinsic if not already declared.
                let fn_ty = LLVMFunctionType(
                    agg_ty,
                    [elem_ty, elem_ty].as_ptr() as *mut LLVMTypeRef,
                    2,
                    0,
                );
                let name_c = CString::new(name.as_str()).unwrap();
                let intrinsic_fn = if self.values.contains_key(&name) {
                    *self.values.get(&name).unwrap()
                } else {
                    let f = LLVMAddFunction(self.module, name_c.as_ptr(), fn_ty);
                    self.values.insert(name, f);
                    f
                };

                // Call the intrinsic: %r = call { T, i1 } @intrinsic(T %lhs, T %rhs)
                let lhs_val = self.lookup(lhs);
                let rhs_val = self.lookup(rhs);
                let mut args = [lhs_val, rhs_val];
                let name_c = CString::new("cbo").unwrap();
                // Stage 14.103: LLVMBuildCall2 requires the FUNCTION type (fn_ty),
                // NOT the return type (agg_ty). Passing agg_ty caused segfaults.
                let r = LLVMBuildCall2(
                    self.builder,
                    fn_ty,
                    intrinsic_fn,
                    args.as_mut_ptr(),
                    2,
                    name_c.as_ptr(),
                );
                return self.fresh_named(r);
            }

            // Unsupported op or type — fall back to "no overflow".
            // Synthesize `{ T, i1 } undef` with the overflow flag zeroed.
            let agg = LLVMGetUndef(agg_ty);
            let zero_i1 = LLVMConstInt(i1_ty, 0, 0);
            let name_c = CString::new("cbo").unwrap();
            let r = LLVMBuildInsertValue(self.builder, agg, zero_i1, 1, name_c.as_ptr());
            self.fresh_named(r)
        }
    }
}
