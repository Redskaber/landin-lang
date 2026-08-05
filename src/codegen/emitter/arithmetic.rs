//! ArithmeticEmitter sub-trait — value computation from operands.
//!
//! Stage 16.76 MUV-1: Split from the original 39-method `Emitter` trait
//! (single responsibility per §13.4 J2). ArithmeticEmitter owns the 11
//! methods that compute a value from one or more operand values:
//! constants, binary/unary ops, comparisons, bitwise ops, casts, selects,
//! and checked-binop intrinsics.

use crate::mir::place::{BinOp, UnOp};
use crate::mir::ty::ConstVal;

use super::{EmitType, EmitValue};

/// Arithmetic / value-computation emission.
///
/// Per §13.4 J2 single responsibility: this trait covers everything that
/// produces a new value from one or more operand values — constants,
/// arithmetic, comparisons, bitwise ops, type conversions, and selects.
pub trait ArithmeticEmitter {
    /// Emit a constant value and return its handle.
    fn emit_const(&mut self, val: &ConstVal) -> EmitValue;

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

    /// Emit an integer comparison (icmp).
    fn emit_icmp(&mut self, op: &str, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue)
        -> EmitValue;

    /// Emit a float comparison (fcmp).
    fn emit_fcmp(&mut self, op: &str, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue)
        -> EmitValue;

    /// Emit a bitwise AND.
    fn emit_and(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;

    /// Emit a bitwise OR.
    fn emit_or(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;

    /// Emit a zero-extend (zext).
    fn emit_zext(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue;

    /// Emit a type cast.
    fn emit_cast(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue;

    /// Emit a `select` instruction.
    fn emit_select(
        &mut self,
        ty: &EmitType,
        cond: &EmitValue,
        true_val: &EmitValue,
        false_val: &EmitValue,
    ) -> EmitValue;

    /// Emit a checked-binary-op intrinsic call.
    fn emit_checked_binop(
        &mut self,
        op: BinOp,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue;
}
