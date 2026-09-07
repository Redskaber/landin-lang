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
    ///
    /// # Stage 145 (TD-CODEGEN-CAST-UNSIGNED) — `src_signed` parameter
    ///
    /// `EmitType` only carries integer *width* (I8/I16/I32/I64), not
    /// *signedness* — both `i8` and `u8` map to `EmitType::I8`. For casts
    /// that *widen* (e.g., `u8 as i64`), the choice between sign-extension
    /// (sext, for signed) and zero-extension (zext, for unsigned) depends
    /// on the source type's signedness, which `EmitType` cannot provide.
    ///
    /// Per §1.0 原則 10 (唯一可信数据源): the caller — which has access to
    /// MIR `Ty` (the authoritative source of signedness) — must pass
    /// `src_signed: bool` to this method.
    /// Per §1.0 原則 6 (通解 > 特解): one `emit_cast` handles both signed
    /// and unsigned sources based on the parameter — no separate
    /// `emit_cast_unsigned` method.
    /// Per §1.0 原則 5 (去除兼容思维): the old signature is replaced.
    ///
    /// # Parameter semantics
    ///
    /// - `src_signed = true`: source is a signed integer (`i8`/`i16`/`i32`/
    ///   `i64`/`isize`). Widening uses `sext` (sign-extend).
    /// - `src_signed = false`: source is an unsigned integer (`u8`/`u16`/
    ///   `u32`/`u64`/`usize`). Widening uses `zext` (zero-extend).
    /// - For non-integer sources (float, ptr), `src_signed` is ignored.
    fn emit_cast(
        &mut self,
        src: &EmitType,
        dst: &EmitType,
        src_signed: bool,
        val: &EmitValue,
    ) -> EmitValue;

    /// Stage 18.205 (TD-FUNCTION-REDEFINE-PARAMS fix): Emit a null pointer
    /// constant (`ptr null`). Used for `ConstVal::Int(0)` in pointer-typed
    /// contexts to avoid a LLVM backend optimization that collapses
    /// `store ptr null` to a 4-byte `store i32 0`, leaving upper bytes
    /// uninitialized and causing ABI mismatches on 8-byte loads.
    ///
    /// Per §12 (最优 > 最小): emit the right constant type upfront.
    fn emit_null_ptr(&mut self) -> EmitValue;

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

    /// Stage 18.287 (TD-NEGOVERFLOW-I32 fix): Emit a typed integer constant.
    ///
    /// Unlike `emit_const(&ConstVal::Int(0))` (which defaults to i32 for
    /// small values), this method emits the constant with the EXACT type
    /// specified by `ty`. This is needed for overflow asserts where the
    /// operand's type must match the zero constant's type (e.g., i64 operand
    /// needs `i64 0`, not `i32 0`).
    ///
    /// Per §1.0 原則 6 (通解 > 特解): one typed-const method for all int widths.
    /// Per §12 (最优 > 最小): fix root cause (typed const), not symptom (cast).
    fn emit_const_typed(&mut self, val: i64, ty: &EmitType) -> EmitValue;
}
