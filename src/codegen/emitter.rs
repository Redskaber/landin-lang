//! Emitter trait: abstracts the codegen backend.
//!
//! This trait decouples the MIR → IR translation logic from the specific
//! backend (text .ll, inkwell, Cranelift). The translation walks MIR and
//! calls Emitter methods; the backend implements those methods.
//!
//! Design principle (per DeepSeek's feedback):
//!   "先设计一套中间表示，让 .ll 文本生成和未来的 Inkwell 生成都
//!    依赖于这套 IR，这样迁移时只替换最底层的 Emitter。"
//!
//! We use MIR itself as the "middle IR" — no need for a separate MLIR-like
//! Operation layer. The Emitter trait is the abstraction boundary.

use crate::mir::lvalue::{BinOp, UnOp};
use crate::mir::ty::ConstVal;

/// A value produced by the emitter — opaque to the translation layer.
///
/// For the text backend, this is a String like "42" or "%v1".
/// For inkwell, this would be IntValue/FloatValue/etc.
/// For Cranelift, this would be cranelift_codegen::ir::Value.
pub type EmitValue = String;

/// The type of a value — used to select the right instruction.
///
/// For the text backend, this maps to LLVM type strings ("i32", "i1", etc.).
/// For inkwell, this would map to inkwell::types::IntType/etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitType {
    I32,
    I64,
    I1, // bool
    F64,
    F32,
    I8, // char
    Ptr,
    Void,
}

/// Abstract emitter trait.
///
/// Each method corresponds to one LLVM IR construct. The translation
/// layer (codegen.rs) calls these methods without knowing whether
/// the output is text, inkwell IR, or something else.
pub trait Emitter {
    /// Begin a new function definition.
    fn begin_function(&mut self, name: &str, params: &[(EmitType, &str)], ret: EmitType);

    /// End the current function definition.
    fn end_function(&mut self);

    /// Emit a constant value and return its handle.
    fn emit_constant(&mut self, val: &ConstVal) -> EmitValue;

    /// Emit a binary operation and return the result value.
    fn emit_binary_op(
        &mut self,
        op: BinOp,
        ty: EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue;

    /// Emit a unary operation and return the result value.
    fn emit_unary_op(&mut self, op: UnOp, ty: EmitType, operand: &EmitValue) -> EmitValue;

    /// Emit a return instruction.
    fn emit_return(&mut self, ty: EmitType, val: Option<&EmitValue>);

    /// Emit an unreachable instruction.
    fn emit_unreachable(&mut self);

    /// Allocate stack space for a local variable.
    fn emit_alloca(&mut self, ty: EmitType, name: &str) -> EmitValue;

    /// Store a value to a pointer.
    fn emit_store(&mut self, ty: EmitType, val: &EmitValue, ptr: &EmitValue);

    /// Load a value from a pointer.
    fn emit_load(&mut self, ty: EmitType, ptr: &EmitValue) -> EmitValue;

    /// Emit a branch to a named label.
    fn emit_branch(&mut self, label: &str);

    /// Emit a conditional branch.
    fn emit_cond_branch(&mut self, cond: &EmitValue, then_label: &str, else_label: &str);

    /// Begin a new basic block with the given label.
    fn begin_block(&mut self, label: &str);

    /// Emit a function call.
    fn emit_call(&mut self, fn_name: &str, args: &[EmitValue], ret_ty: EmitType) -> EmitValue;

    /// Get a reference to a local variable (for load/store).
    /// Returns the value handle for the local's address.
    fn local_ptr(&self, local_id: u32) -> Option<&EmitValue>;

    /// Store a local's pointer handle (alloca result).
    fn set_local_ptr(&mut self, local_id: u32, ptr: EmitValue);

    /// Get a local's pointer handle.
    fn get_local_ptr(&self, local_id: u32) -> Option<&EmitValue>;

    /// Store a local's value handle (for later lookups).
    fn set_local(&mut self, local_id: u32, val: EmitValue);

    /// Get a local's stored value handle.
    fn get_local(&self, local_id: u32) -> Option<&EmitValue>;

    /// Emit a comparison (icmp) and return the i1 result.
    fn emit_icmp(&mut self, op: &str, ty: EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;

    /// Emit a zext (zero extend) from i1 to i32.
    fn emit_zext_i1_to_i32(&mut self, val: &EmitValue) -> EmitValue;

    /// Emit a switch instruction (for match on integers).
    /// Each case is (value, label). The default label is used for `_`.
    fn emit_switch(&mut self, discr: &EmitValue, cases: &[(i128, String)], default_label: &str);

    /// Return the accumulated output (for text backends).
    fn output(&self) -> &str;
}

/// Map a MIR Ty to an EmitType.
pub fn mir_type_to_emit_type(ty: &crate::mir::ty::Ty) -> EmitType {
    use crate::mir::ty::TyKind;
    match &ty.kind {
        TyKind::Int(_) | TyKind::Uint(_) => EmitType::I32,
        TyKind::Bool => EmitType::I1,
        TyKind::Float(crate::ast::FloatTy::F32) => EmitType::F32,
        TyKind::Float(_) => EmitType::F64,
        TyKind::Char => EmitType::I8,
        TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => EmitType::Ptr,
        _ => EmitType::I32, // default
    }
}

/// Map a BinOp + EmitType to the LLVM instruction string (for text backend).
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
    }
}
