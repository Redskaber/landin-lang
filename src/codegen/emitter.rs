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
    I32,
    I64,
    I1,
    F64,
    F32,
    I8,
    /// Typed pointer to a pointee. Falls back to opaque `i8*` when the
    /// pointee is unknown (kept for legacy callers).
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
pub fn mir_type_to_emit_type(ty: &crate::mir::ty::Ty) -> EmitType {
    use crate::mir::ty::TyKind;
    match &ty.kind {
        TyKind::Int(crate::ast::IntTy::I64) | TyKind::Uint(crate::ast::UintTy::U64) => {
            EmitType::I64
        }
        TyKind::Int(crate::ast::IntTy::I128) | TyKind::Uint(crate::ast::UintTy::U128) => {
            EmitType::I64 // simplified — Stage 3 doesn't have i128 yet
        }
        TyKind::Int(_) | TyKind::Uint(_) => EmitType::I32,
        TyKind::Bool => EmitType::I1,
        TyKind::Float(crate::ast::FloatTy::F32) => EmitType::F32,
        TyKind::Float(_) => EmitType::F64,
        TyKind::Char => EmitType::I8,
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => {
            EmitType::ptr_to(mir_type_to_emit_type(inner))
        }
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
pub fn binop_to_llvm_str(op: BinOp, ty: &EmitType) -> String {
    match (op, ty) {
        (BinOp::Add, EmitType::I32) => "add nsw i32".into(),
        (BinOp::Add, EmitType::I64) => "add nsw i64".into(),
        (BinOp::Sub, EmitType::I32) => "sub nsw i32".into(),
        (BinOp::Sub, EmitType::I64) => "sub nsw i64".into(),
        (BinOp::Mul, EmitType::I32) => "mul nsw i32".into(),
        (BinOp::Mul, EmitType::I64) => "mul nsw i64".into(),
        (BinOp::Div, EmitType::I32) => "sdiv i32".into(),
        (BinOp::Div, EmitType::I64) => "sdiv i64".into(),
        (BinOp::Rem, EmitType::I32) => "srem i32".into(),
        (BinOp::Rem, EmitType::I64) => "srem i64".into(),
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
        (BinOp::BitAnd, EmitType::I32) => "and i32".into(),
        (BinOp::BitAnd, EmitType::I1) => "and i1".into(),
        (BinOp::BitAnd, EmitType::I64) => "and i64".into(),
        (BinOp::BitOr, EmitType::I32) => "or i32".into(),
        (BinOp::BitOr, EmitType::I1) => "or i1".into(),
        (BinOp::BitOr, EmitType::I64) => "or i64".into(),
        (BinOp::BitXor, EmitType::I32) => "xor i32".into(),
        (BinOp::BitXor, EmitType::I1) => "xor i1".into(),
        (BinOp::BitXor, EmitType::I64) => "xor i64".into(),
        (BinOp::Shl, EmitType::I32) => "shl i32".into(),
        (BinOp::Shl, EmitType::I64) => "shl i64".into(),
        (BinOp::Shr, EmitType::I32) => "ashr i32".into(),
        (BinOp::Shr, EmitType::I64) => "ashr i64".into(),
        // Floats don't have bitwise ops; bitcast to int of same width then back.
        // For Stage 3 we just fall back to the int form (caller should avoid this).
        _ => "add i32".into(),
    }
}

/// Map an EmitType to its LLVM type string.
///
/// Stage 3.21: returns `String` (was `&'static str`) because struct and
/// array layouts must be rendered dynamically from their element types.
pub fn emit_type_to_llvm_str(ty: &EmitType) -> String {
    match ty {
        EmitType::I32 => "i32".into(),
        EmitType::I64 => "i64".into(),
        EmitType::I1 => "i1".into(),
        EmitType::F64 => "double".into(),
        EmitType::F32 => "float".into(),
        EmitType::I8 => "i8".into(),
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
