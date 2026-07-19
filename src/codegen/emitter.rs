//! Emitter trait: abstracts the codegen backend.
//!
//! Naming conventions (per §14 review):
//! - All IR-producing methods use `emit_` prefix
//! - State query methods use `get_` / `set_` prefix
//! - Function/block scope methods use `emit_` prefix for consistency
//! - No redundant suffixes (e.g. `_typed` when there's only one variant)
//! - `local_ptr` removed — `get_local_ptr` is the only accessor

use crate::mir::lvalue::{BinOp, UnOp};
use crate::mir::ty::ConstVal;

/// A value produced by the emitter — opaque to the translation layer.
pub type EmitValue = String;

/// The type of a value — used to select the right instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitType {
    I32,
    I64,
    I1,
    F64,
    F32,
    I8,
    Ptr,
    Void,
    Tuple,
    Array,
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
    fn emit_function_begin(&mut self, name: &str, params: &[(EmitType, &str)], ret: EmitType);

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
        ty: EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue;

    /// Emit a unary operation and return the result value.
    fn emit_unop(&mut self, op: UnOp, ty: EmitType, operand: &EmitValue) -> EmitValue;

    // === Control flow ===

    /// Emit a return instruction.
    fn emit_ret(&mut self, ty: EmitType, val: Option<&EmitValue>);

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
        discr_ty: EmitType,
        cases: &[(i128, String)],
        default_label: &str,
    );

    // === Memory ===

    /// Allocate stack space for a local variable.
    fn emit_alloca(&mut self, ty: EmitType, name: &str) -> EmitValue;

    /// Store a value to a pointer.
    fn emit_store(&mut self, ty: EmitType, val: &EmitValue, ptr: &EmitValue);

    /// Load a value from a pointer.
    fn emit_load(&mut self, ty: EmitType, ptr: &EmitValue) -> EmitValue;

    // === Calls ===

    /// Emit a function call.
    fn emit_call(&mut self, fn_name: &str, args: &[EmitValue], ret_ty: EmitType) -> EmitValue;

    // === Comparisons ===

    /// Emit an integer comparison (icmp).
    fn emit_icmp(&mut self, op: &str, ty: EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;

    /// Emit a float comparison (fcmp).
    fn emit_fcmp(&mut self, op: &str, ty: EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;

    // === Type conversions ===

    /// Emit a zero-extend (zext) from one type to another.
    fn emit_zext(&mut self, src: EmitType, dst: EmitType, val: &EmitValue) -> EmitValue;

    /// Emit a type cast (trunc/sext/zext/sitofp/fptosi/fpext/fptrunc).
    fn emit_cast(&mut self, src: EmitType, dst: EmitType, val: &EmitValue) -> EmitValue;

    // === Aggregates ===

    /// Emit a getelementptr for struct field access.
    fn emit_gep_field(&mut self, base_ptr: &EmitValue, field_index: u32) -> EmitValue;

    /// Emit a getelementptr for array index access.
    fn emit_gep_index(&mut self, base_ptr: &EmitValue, index: &EmitValue) -> EmitValue;

    /// Emit a PHI node for merging values from multiple predecessor blocks.
    fn emit_phi(&mut self, ty: EmitType, incoming: &[(EmitValue, String)]) -> EmitValue;

    /// Emit insertvalue for tuple/struct construction.
    fn emit_insertvalue(
        &mut self,
        agg_ty: EmitType,
        agg: &EmitValue,
        val: &EmitValue,
        index: u32,
    ) -> EmitValue;

    /// Emit extractvalue for tuple/struct field extraction.
    fn emit_extractvalue(&mut self, agg_ty: EmitType, agg: &EmitValue, index: u32) -> EmitValue;

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
pub fn mir_type_to_emit_type(ty: &crate::mir::ty::Ty) -> EmitType {
    use crate::mir::ty::TyKind;
    match &ty.kind {
        TyKind::Int(crate::ast::IntTy::I64) | TyKind::Uint(crate::ast::UintTy::U64) => {
            EmitType::I64
        }
        TyKind::Int(crate::ast::IntTy::I128) | TyKind::Uint(crate::ast::UintTy::U128) => {
            EmitType::I64 // simplified
        }
        TyKind::Int(_) | TyKind::Uint(_) => EmitType::I32,
        TyKind::Bool => EmitType::I1,
        TyKind::Float(crate::ast::FloatTy::F32) => EmitType::F32,
        TyKind::Float(_) => EmitType::F64,
        TyKind::Char => EmitType::I8,
        TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => EmitType::Ptr,
        TyKind::Tuple(tys) => {
            if tys.is_empty() {
                EmitType::Void
            } else {
                EmitType::Tuple
            }
        }
        TyKind::Array(_, _) => EmitType::Array,
        _ => EmitType::I32,
    }
}

/// Map a BinOp + EmitType to the LLVM instruction string.
pub fn binop_to_llvm_str(op: BinOp, ty: EmitType) -> &'static str {
    match (op, ty) {
        (BinOp::Add, EmitType::I32) => "add nsw i32",
        (BinOp::Add, EmitType::I64) => "add nsw i64",
        (BinOp::Sub, EmitType::I32) => "sub nsw i32",
        (BinOp::Sub, EmitType::I64) => "sub nsw i64",
        (BinOp::Mul, EmitType::I32) => "mul nsw i32",
        (BinOp::Mul, EmitType::I64) => "mul nsw i64",
        (BinOp::Div, EmitType::I32) => "sdiv i32",
        (BinOp::Div, EmitType::I64) => "sdiv i64",
        (BinOp::Rem, EmitType::I32) => "srem i32",
        (BinOp::Rem, EmitType::I64) => "srem i64",
        (BinOp::Add, EmitType::F64) => "fadd double",
        (BinOp::Add, EmitType::F32) => "fadd float",
        (BinOp::Sub, EmitType::F64) => "fsub double",
        (BinOp::Sub, EmitType::F32) => "fsub float",
        (BinOp::Mul, EmitType::F64) => "fmul double",
        (BinOp::Mul, EmitType::F32) => "fmul float",
        (BinOp::Div, EmitType::F64) => "fdiv double",
        (BinOp::Div, EmitType::F32) => "fdiv float",
        (BinOp::Rem, EmitType::F64) => "frem double",
        (BinOp::Rem, EmitType::F32) => "frem float",
        (BinOp::BitAnd, EmitType::I32) => "and i32",
        (BinOp::BitAnd, EmitType::I1) => "and i1",
        (BinOp::BitOr, EmitType::I32) => "or i32",
        (BinOp::BitOr, EmitType::I1) => "or i1",
        (BinOp::BitXor, EmitType::I32) => "xor i32",
        (BinOp::BitXor, EmitType::I1) => "xor i1",
        (BinOp::Shl, _) => "shl i32",
        (BinOp::Shr, _) => "ashr i32",
        _ => "add i32",
    }
}

/// Map an EmitType to its LLVM type string.
pub fn emit_type_to_llvm_str(ty: EmitType) -> &'static str {
    match ty {
        EmitType::I32 => "i32",
        EmitType::I64 => "i64",
        EmitType::I1 => "i1",
        EmitType::F64 => "double",
        EmitType::F32 => "float",
        EmitType::I8 => "i8",
        EmitType::Ptr => "i32*",
        EmitType::Void => "void",
        EmitType::Tuple => "{ i32 }",
        EmitType::Array => "[10 x i32]",
    }
}
