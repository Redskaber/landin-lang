//! Emitter trait: abstracts the codegen backend.
//!
//! Naming conventions (per §14 review):
//! - All IR-producing methods use `emit_` prefix
//! - State query methods use `get_` / `set_` prefix
//! - Function/block scope methods use `emit_` prefix for consistency
//! - No redundant suffixes (e.g. `_typed` when there's only one variant)
//! - `local_ptr` removed — `get_local_ptr` is the only accessor
//!
//! Stage 3.21 (v0.8.6): EmitType now carries full structure:
//! - `Struct(Vec<EmitType>)` for tuples / structs
//! - `Array(Box<EmitType>, u64)` for `[T; N]`
//! - `Ptr(Box<EmitType>)` for typed pointers (pointee tracked)
//!
//! `emit_type_to_llvm_str` returns `String` (was `&'static str`) so that
//! dynamic struct/array layouts can be rendered.

use crate::mir::lvalue::{BinOp, UnOp};
use crate::mir::ty::ConstVal;

/// A value produced by the emitter — opaque to the translation layer.
pub type EmitValue = String;

/// The type of a value — used to select the right instruction.
///
/// Stage 3.21: made non-Copy (now carries `Vec`/`Box`) so struct and array
/// layouts can be fully represented. Callers pass `&EmitType` to emitters.
#[derive(Debug, Clone, PartialEq)]
pub enum EmitType {
    I1,
    I8,
    I16,
    I32,
    I64,
    I128,
    F32,
    F64,
    /// Typed pointer to a pointee.
    Ptr(Box<EmitType>),
    /// Opaque pointer — no pointee info available.
    OpaquePtr,
    Void,
    /// Struct / tuple: ordered field types.
    Struct(Vec<EmitType>),
    /// Array `[T; N]`: element type + length.
    Array(Box<EmitType>, u64),
}

// Convenience constructors (keep call-sites readable).
impl EmitType {
    pub fn ptr_to(pointee: EmitType) -> Self {
        EmitType::Ptr(Box::new(pointee))
    }

    pub fn struct_of(fields: Vec<EmitType>) -> Self {
        EmitType::Struct(fields)
    }

    pub fn array_of(elem: EmitType, len: u64) -> Self {
        EmitType::Array(Box::new(elem), len)
    }

    /// Return the LLVM type used to dereference this pointer.
    /// For `Ptr(t)` returns `&t`; for `OpaquePtr` returns `i32` (legacy
    /// default — most loads in old code paths assumed i32).
    pub fn pointee(&self) -> EmitType {
        match self {
            EmitType::Ptr(t) => (**t).clone(),
            EmitType::OpaquePtr => EmitType::I32,
            other => other.clone(),
        }
    }

    /// True if this is a pointer (typed or opaque).
    pub fn is_ptr(&self) -> bool {
        matches!(self, EmitType::Ptr(_) | EmitType::OpaquePtr)
    }
}

/// Abstract emitter trait.
///
/// Naming conventions:
/// - `emit_*`: produces IR instructions, may return EmitValue
/// - `get_*` / `set_*`: queries or updates emitter state
/// - `emit_declare_*`: module-level declarations
pub trait Emitter {
    // === Module-level ===

    /// Emit module header (target triple, datalayout).
    fn emit_header(&mut self);

    /// Emit an external function declaration.
    fn emit_declare(&mut self, signature: &str);

    // === Function scope ===

    /// Begin a new function definition.
    fn emit_function_begin(&mut self, name: &str, params: &[(EmitType, &str)], ret: &EmitType);

    /// End the current function definition.
    fn emit_function_end(&mut self);

    // === Constants ===

    /// Emit a constant value and return its handle.
    fn emit_const(&mut self, val: &ConstVal) -> EmitValue;

    // === Arithmetic ===

    /// Emit a binary operation and return the result value.
    fn emit_binop(
        &mut self,
        op: BinOp,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue;

    /// Emit a unary operation and return the result value.
    fn emit_unop(&mut self, op: UnOp, ty: &EmitType, operand: &EmitValue) -> EmitValue;

    // === Control flow ===

    /// Emit a return instruction.
    fn emit_ret(&mut self, ty: &EmitType, val: Option<&EmitValue>);

    /// Emit an unreachable instruction.
    fn emit_unreachable(&mut self);

    /// Emit an unconditional branch to a label.
    fn emit_br(&mut self, label: &str);

    /// Emit a conditional branch.
    fn emit_br_cond(&mut self, cond: &EmitValue, then_label: &str, else_label: &str);

    /// Begin a new basic block with the given label.
    fn emit_block(&mut self, label: &str);

    /// Emit a switch instruction (typed: i32 or i64).
    fn emit_switch(
        &mut self,
        discr: &EmitValue,
        discr_ty: &EmitType,
        cases: &[(i128, String)],
        default_label: &str,
    );

    // === Memory ===

    /// Allocate stack space for a local variable.
    fn emit_alloca(&mut self, ty: &EmitType, name: &str) -> EmitValue;

    /// Store a value to a pointer.
    fn emit_store(&mut self, ty: &EmitType, val: &EmitValue, ptr: &EmitValue);

    /// Load a value from a pointer.
    fn emit_load(&mut self, ty: &EmitType, ptr: &EmitValue) -> EmitValue;

    // === Calls ===

    /// Emit a function call with typed arguments.
    fn emit_call(
        &mut self,
        fn_name: &str,
        args: &[(EmitType, &EmitValue)],
        ret_ty: &EmitType,
    ) -> EmitValue;

    // === Comparisons ===

    /// Emit an integer comparison (icmp).
    fn emit_icmp(&mut self, op: &str, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue)
        -> EmitValue;

    /// Emit a float comparison (fcmp).
    fn emit_fcmp(&mut self, op: &str, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue)
        -> EmitValue;

    /// Stage 3.49 (L13 closure): Emit a bitwise AND (`and ty lhs, rhs`).
    /// Used for fat-pointer equality comparison (AND of ptr-eq and len-eq).
    fn emit_and(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;

    /// Stage 3.49 (L13 closure): Emit a bitwise OR (`or ty lhs, rhs`).
    /// Used for fat-pointer inequality comparison (OR of ptr-ne and len-ne).
    fn emit_or(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;

    // === Type conversions ===

    /// Emit a zero-extend (zext) from one type to another.
    fn emit_zext(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue;

    /// Emit a type cast (trunc/sext/zext/sitofp/fptosi/fpext/fptrunc).
    fn emit_cast(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue;

    // === Aggregates ===

    /// Emit a getelementptr for struct field access.
    /// `struct_ty` is the struct's LLVM type (used to render the GEP).
    fn emit_gep_field(
        &mut self,
        base_ptr: &EmitValue,
        struct_ty: &EmitType,
        field_index: u32,
    ) -> EmitValue;

    /// Emit a getelementptr for array index access.
    /// `array_ty` is the array's LLVM type (`[N x T]`).
    fn emit_gep_index(
        &mut self,
        base_ptr: &EmitValue,
        array_ty: &EmitType,
        index: &EmitValue,
    ) -> EmitValue;

    /// Stage 3.51: Emit a getelementptr for element access via a raw
    /// element pointer (not an array pointer). Used for slice indexing
    /// where the data pointer is `T*` (not `[N x T]*`).
    ///
    /// Emits: `%r = getelementptr inbounds <elem_ty>, <elem_ty>* %base, i32 %idx`
    fn emit_gep_index_ptr(
        &mut self,
        base_ptr: &EmitValue,
        elem_ty: &EmitType,
        index: &EmitValue,
    ) -> EmitValue;

    /// Emit a PHI node for merging values from multiple predecessor blocks.
    fn emit_phi(&mut self, ty: &EmitType, incoming: &[(EmitValue, String)]) -> EmitValue;

    /// Emit insertvalue for tuple/struct construction.
    /// `val_ty` is the type of the value being inserted (so we render it correctly).
    fn emit_insertvalue(
        &mut self,
        agg_ty: &EmitType,
        agg: &EmitValue,
        val_ty: &EmitType,
        val: &EmitValue,
        index: u32,
    ) -> EmitValue;

    /// Emit extractvalue for tuple/struct field extraction.
    fn emit_extractvalue(&mut self, agg_ty: &EmitType, agg: &EmitValue, index: u32) -> EmitValue;

    /// Emit a checked-binary-op intrinsic call (e.g.
    /// `llvm.sadd.with.overflow.i32`) and return the aggregate result
    /// `{T, i1}`. Caller then `extractvalue`s index 1 for the overflow flag.
    ///
    /// Stage 3.24: only Add/Sub/Mul on i32/i64 are supported (matching
    /// the LLVM intrinsic family). Other ops return `undef` of the right
    /// aggregate type with i1 = 0 (i.e., assume no overflow).
    fn emit_checked_binop(
        &mut self,
        op: BinOp,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue;

    /// Emit (or look up) a module-level string constant global and return
    /// its symbolic name (e.g. `@.str.0`).
    ///
    /// Stage 3.27: emitted as `@.str.N = private unnamed_addr constant [M x i8] c"..."`
    /// at module scope. Returns the global's name (without leading `@`).
    /// The same content should yield the same global (deduplicated) so that
    /// repeated literals don't bloat the module.
    ///
    /// `bytes` is the raw byte content (no null terminator added — caller
    /// controls the encoding).
    fn emit_string_global(&mut self, bytes: &[u8]) -> EmitValue;

    // === Local state ===

    /// Store a local's pointer handle (alloca result).
    fn set_local_ptr(&mut self, local_id: u32, ptr: EmitValue);

    /// Get a local's pointer handle.
    fn get_local_ptr(&self, local_id: u32) -> Option<&EmitValue>;

    /// Store a local's value handle (for later lookups).
    fn set_local(&mut self, local_id: u32, val: EmitValue);

    /// Get a local's stored value handle.
    fn get_local(&self, local_id: u32) -> Option<&EmitValue>;

    // === Output ===

    /// Return the accumulated output (for text backends).
    fn output(&self) -> &str;
}

// ================================================================
// Type mapping helpers
// ================================================================

/// Map a MIR Ty to an EmitType.
///
/// Stage 3.21: now produces proper `Struct` / `Array` / `Ptr` variants
/// (was hardcoded to `Tuple` / `Array` / `Ptr` opaque).
/// Stage 3.49 (L13 closure): Construct the EmitType for a fat pointer
/// (`{ ptr_to_elem, i64 }`). Used for `&str` and `&[T]` references,
/// which carry both a data pointer and a length.
///
/// Per §15 (最优 > 最小): fat pointers are the architecturally correct
/// representation for references to unsized types (`str`, `[T]`). The
/// previous thin-pointer model (Stage 3.27/3.28) lost the length
/// component, making it impossible to recover the length of a `&str`
/// after passing it to a function — a soundness/completeness gap
/// carried as L13 debt since Stage 3.27 (18 rounds).
pub fn fat_ptr_type(elem: EmitType) -> EmitType {
    EmitType::Struct(vec![EmitType::ptr_to(elem), EmitType::I64])
}

pub fn mir_type_to_emit_type(ty: &crate::mir::ty::Ty) -> EmitType {
    use crate::mir::ty::TyKind;
    match &ty.kind {
        TyKind::Int(crate::ast::IntTy::I8) | TyKind::Uint(crate::ast::UintTy::U8) => EmitType::I8,
        TyKind::Int(crate::ast::IntTy::I16) | TyKind::Uint(crate::ast::UintTy::U16) => {
            EmitType::I16
        }
        TyKind::Int(crate::ast::IntTy::I32) | TyKind::Uint(crate::ast::UintTy::U32) => {
            EmitType::I32
        }
        TyKind::Int(crate::ast::IntTy::I64) | TyKind::Uint(crate::ast::UintTy::U64) => {
            EmitType::I64
        }
        TyKind::Int(crate::ast::IntTy::I128) | TyKind::Uint(crate::ast::UintTy::U128) => {
            EmitType::I128
        }
        TyKind::Int(crate::ast::IntTy::Isize) | TyKind::Uint(crate::ast::UintTy::Usize) => {
            EmitType::I64
        }
        // All explicit IntTy/UintTy variants are covered above.
        // This catch-all is unreachable but kept for safety.
        #[allow(unreachable_patterns)]
        TyKind::Int(_) | TyKind::Uint(_) => EmitType::I32,
        TyKind::Bool => EmitType::I1,
        TyKind::Float(crate::ast::FloatTy::F32) => EmitType::F32,
        TyKind::Float(_) => EmitType::F64,
        TyKind::Char => EmitType::I8,
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => {
            // Stage 3.49 (L13 closure): `&str` and `&[T]` are fat pointers
            // `{ ptr, len }`. Other references remain thin pointers.
            match &inner.kind {
                TyKind::Str => fat_ptr_type(EmitType::I8),
                TyKind::Slice(elem) => fat_ptr_type(mir_type_to_emit_type(elem)),
                _ => EmitType::ptr_to(mir_type_to_emit_type(inner)),
            }
        }
        // Stage 3.49: `Str` and `Slice(T)` are unsized types — they cannot
        // appear as values, only behind a reference (`&str`, `&[T]`). We
        // keep their direct EmitType as thin pointers for internal use
        // (e.g., global string types), but the reference type is now a
        // fat pointer (see the Ref case above).
        TyKind::Str => EmitType::ptr_to(EmitType::I8),
        TyKind::Slice(elem) => EmitType::ptr_to(mir_type_to_emit_type(elem)),
        TyKind::Tuple(tys) => {
            if tys.is_empty() {
                EmitType::Void
            } else {
                EmitType::Struct(tys.iter().map(mir_type_to_emit_type).collect())
            }
        }
        TyKind::Array(elem, len) => {
            let n = match &len.val {
                ConstVal::Int(n) | ConstVal::Uint(n) => *n as u64,
                _ => 0,
            };
            EmitType::array_of(mir_type_to_emit_type(elem), n)
        }
        // Fns/closures/ADTs/etc. — Stage 3 treats as opaque i32 placeholder.
        _ => EmitType::I32,
    }
}

/// Map a BinOp + EmitType to the LLVM instruction string.
///
/// Stage 3.46: generic integer type support — generates the instruction
/// with the correct type suffix for all integer widths (i8/i16/i32/i64/i128).
pub fn binop_to_llvm_str(op: BinOp, ty: &EmitType) -> String {
    let ty_str = emit_type_to_llvm_str(ty);
    let is_int = matches!(
        ty,
        EmitType::I1
            | EmitType::I8
            | EmitType::I16
            | EmitType::I32
            | EmitType::I64
            | EmitType::I128
    );
    match (op, ty) {
        // Integer arithmetic
        (BinOp::Add, _) if is_int => format!("add nsw {}", ty_str),
        (BinOp::Sub, _) if is_int => format!("sub nsw {}", ty_str),
        (BinOp::Mul, _) if is_int => format!("mul nsw {}", ty_str),
        (BinOp::Div, _) if is_int => format!("sdiv {}", ty_str),
        (BinOp::Rem, _) if is_int => format!("srem {}", ty_str),
        // Float arithmetic
        (BinOp::Add, EmitType::F64) => "fadd double".into(),
        (BinOp::Add, EmitType::F32) => "fadd float".into(),
        (BinOp::Sub, EmitType::F64) => "fsub double".into(),
        (BinOp::Sub, EmitType::F32) => "fsub float".into(),
        (BinOp::Mul, EmitType::F64) => "fmul double".into(),
        (BinOp::Mul, EmitType::F32) => "fmul float".into(),
        (BinOp::Div, EmitType::F64) => "fdiv double".into(),
        (BinOp::Div, EmitType::F32) => "fdiv float".into(),
        (BinOp::Rem, EmitType::F64) => "frem double".into(),
        (BinOp::Rem, EmitType::F32) => "frem float".into(),
        // Bitwise (all integer types)
        (BinOp::BitAnd, _) if is_int => format!("and {}", ty_str),
        (BinOp::BitOr, _) if is_int => format!("or {}", ty_str),
        (BinOp::BitXor, _) if is_int => format!("xor {}", ty_str),
        (BinOp::Shl, _) if is_int => format!("shl {}", ty_str),
        (BinOp::Shr, _) if is_int => format!("ashr {}", ty_str),
        _ => "add i32".into(),
    }
}

/// Map an EmitType to its LLVM type string.
///
/// Stage 3.21: returns `String` (was `&'static str`) because struct and
/// array layouts must be rendered dynamically from their element types.
pub fn emit_type_to_llvm_str(ty: &EmitType) -> String {
    match ty {
        EmitType::I1 => "i1".into(),
        EmitType::I8 => "i8".into(),
        EmitType::I16 => "i16".into(),
        EmitType::I32 => "i32".into(),
        EmitType::I64 => "i64".into(),
        EmitType::I128 => "i128".into(),
        EmitType::F32 => "float".into(),
        EmitType::F64 => "double".into(),
        EmitType::Ptr(pointee) => format!("{}*", emit_type_to_llvm_str(pointee)),
        EmitType::OpaquePtr => "i32*".into(),
        EmitType::Void => "void".into(),
        EmitType::Struct(fields) => {
            if fields.is_empty() {
                "{}".into()
            } else {
                let parts: Vec<String> = fields.iter().map(emit_type_to_llvm_str).collect();
                format!("{{ {} }}", parts.join(", "))
            }
        }
        EmitType::Array(elem, n) => format!("[{} x {}]", n, emit_type_to_llvm_str(elem)),
    }
}

/// Render a pointer-to-`ty` LLVM type string (convenience).
pub fn llvm_ptr_str(ty: &EmitType) -> String {
    format!("{}*", emit_type_to_llvm_str(ty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::TextEmitter;

    /// Stage 3.57 (P1-5): Verify TextEmitter implements the Emitter trait.
    /// This is a compile-time check — if Emitter trait changes and
    /// TextEmitter doesn't keep up, this test fails to compile.
    #[test]
    fn text_emitter_satisfies_emitter_trait() {
        let _: &dyn Emitter = &TextEmitter::new();
    }

    /// Stage 3.57: Verify emit_type_to_llvm_str roundtrips for key types.
    #[test]
    fn emit_type_to_llvm_str_roundtrips() {
        assert_eq!(emit_type_to_llvm_str(&EmitType::I32), "i32");
        assert_eq!(emit_type_to_llvm_str(&EmitType::I64), "i64");
        assert_eq!(emit_type_to_llvm_str(&EmitType::F64), "double");
        assert_eq!(emit_type_to_llvm_str(&EmitType::Void), "void");
        assert_eq!(
            emit_type_to_llvm_str(&EmitType::Struct(vec![EmitType::I32, EmitType::I64])),
            "{ i32, i64 }"
        );
        assert_eq!(
            emit_type_to_llvm_str(&EmitType::Array(Box::new(EmitType::I8), 5)),
            "[5 x i8]"
        );
    }

    /// Stage 3.57: Verify fat_ptr_type produces the correct { Ptr, I64 } shape.
    #[test]
    fn fat_ptr_type_correct_shape() {
        let fp = fat_ptr_type(EmitType::I8);
        match fp {
            EmitType::Struct(fields) => {
                assert_eq!(fields.len(), 2);
                assert!(fields[0].is_ptr());
                assert_eq!(fields[1], EmitType::I64);
            }
            _ => panic!("expected Struct, got {:?}", fp),
        }
    }

    /// Stage 3.57: Verify mir_type_to_emit_type for basic types.
    #[test]
    fn mir_type_to_emit_type_correct() {
        let i32_ty = crate::mir::ty::Ty::new(
            crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32),
            crate::session::Span::DUMMY,
        );
        assert_eq!(mir_type_to_emit_type(&i32_ty), EmitType::I32);

        let f64_ty = crate::mir::ty::Ty::new(
            crate::mir::ty::TyKind::Float(crate::ast::FloatTy::F64),
            crate::session::Span::DUMMY,
        );
        assert_eq!(mir_type_to_emit_type(&f64_ty), EmitType::F64);
    }

    /// Stage 3.57: Verify EmitType helper methods (ptr_to, pointee, is_ptr).
    #[test]
    fn emit_type_helpers() {
        let ptr = EmitType::ptr_to(EmitType::I32);
        assert!(ptr.is_ptr());
        assert_eq!(ptr.pointee(), EmitType::I32);

        let struct_ty = EmitType::struct_of(vec![EmitType::I32, EmitType::I64]);
        assert!(!struct_ty.is_ptr());

        let arr = EmitType::array_of(EmitType::I8, 10);
        match arr {
            EmitType::Array(elem, len) => {
                assert_eq!(*elem, EmitType::I8);
                assert_eq!(len, 10);
            }
            _ => panic!("expected Array"),
        }
    }

    /// Stage 3.57: Verify TextEmitter produces non-empty output.
    #[test]
    fn text_emitter_produces_output() {
        let mut emitter = TextEmitter::new();
        emitter.emit_header();
        emitter.emit_declare("void @test()");
        let output = emitter.output_with_globals();
        assert!(!output.is_empty());
        assert!(output.contains("target triple"));
    }
}
