//! Stage 6.4: Overflow/assert helper extraction from mir/lower/mod.rs (TD-011 split step 4).
//!
//! Extracted from `mir/lower/mod.rs` to reduce its LOC (2730 → ~2650).
//! Contains functions for emitting overflow and division-by-zero assert terminators.

use crate::hir::HirBinOp;
use crate::mir::body::{AssertMessage, Terminator};
use crate::mir::place::{BinOp, LocalId, Operand};
use crate::mir::ty::{Const, ConstVal, Ty, TyKind};
use crate::session::Span;

use super::MirLowerCtxt;

/// Whether a HIR binary op can overflow (and thus needs an Assert check).
///
/// Comparison ops (Eq/Ne/Lt/Le/Gt/Ge) and bitwise ops (BitAnd/BitOr/BitXor)
/// cannot overflow. Arithmetic (Add/Sub/Mul/Div/Rem) and shift ops
/// (Shl/Shr) can.
pub(crate) fn is_overflowable_op(op: HirBinOp) -> bool {
    matches!(
        op,
        HirBinOp::Add
            | HirBinOp::Sub
            | HirBinOp::Mul
            | HirBinOp::Div
            | HirBinOp::Rem
            | HirBinOp::Shl
            | HirBinOp::Shr
    )
}

/// Emit an `Assert` terminator that checks for arithmetic overflow.
///
/// Stage 3.24: now carries `lhs` and `rhs` operands in the `Overflow` message
/// so codegen can emit `llvm.{sadd,ssub,smul}.with.overflow.*` intrinsics and
/// branch on the real overflow flag. The `cond` field of the Assert remains
/// `Bool(true)` for backward compatibility with typeck/borrowck (which treat
/// the Assert as a normal terminator) — codegen ignores `cond` for Overflow
/// messages and uses the operands directly.
///
/// The Assert is emitted as the terminator of the current block, and
/// a fresh continuation block is created for the rest of the code.
pub(crate) fn emit_overflow_assert(
    cx: &mut MirLowerCtxt,
    result: LocalId,
    op: BinOp,
    lhs: Operand,
    rhs: Operand,
    span: Span,
) {
    let cont = cx.new_block();
    cx.terminate_and_goto(
        Terminator::Assert {
            cond: Operand::Constant(Const {
                ty: Box::new(Ty::new(TyKind::Bool, span)),
                val: ConstVal::Bool(true),
            }),
            expected: true,
            target: cont,
            msg: AssertMessage::Overflow(op, lhs, rhs),
        },
        cont,
    );
    let _ = result;
}

/// Emit an `Assert` terminator that checks for division by zero.
///
/// Stage 3.25: emitted for `Div` and `Rem` operations. The `rhs` operand
/// is stored in the `DivisionByZero` message so codegen can emit
/// `icmp eq rhs, 0` and branch to a panic block on true.
///
/// `result` is unused (kept for API symmetry with `emit_overflow_assert`).
pub(crate) fn emit_div_by_zero_assert(
    cx: &mut MirLowerCtxt,
    result: LocalId,
    rhs: Operand,
    span: Span,
) {
    let cont = cx.new_block();
    cx.terminate_and_goto(
        Terminator::Assert {
            cond: Operand::Constant(Const {
                ty: Box::new(Ty::new(TyKind::Bool, span)),
                val: ConstVal::Bool(true),
            }),
            expected: true,
            target: cont,
            msg: AssertMessage::DivisionByZero(rhs),
        },
        cont,
    );
    let _ = result;
}
