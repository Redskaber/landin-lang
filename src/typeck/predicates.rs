//! Stage 6.15 (TD-025): Type classification predicates + coercion rules.
//!
//! Per 03-type-system.md §8 (Subtyping rules) + §4.4 (Constraint generation).
//! Extracted from `checker.rs` per `docs/stage-committee-process.md` v3.21
//! §14.4 + §13.4.
//!
//! Owns 5 type predicate / coercion functions:
//! - `is_arithmetic_ty` (Int/Uint/Float/Infer/Error — for Add/Sub/Mul/Div/Rem)
//! - `is_negatable_ty` (unary `-`)
//! - `is_notable_ty` (bitwise `!`)
//! - `is_shift_count_ty` (rhs of Shl/Shr; also lhs per Stage 18.412)
//! - `can_coerce` (implicit coercion matrix per §8)
//!
//! Stage 18.413 (v0.5+ Phase 2 L3 step 2): REMOVED `is_concrete_int_or_float`
//! — was only used by the removed `writeback_binaryop_results` (Pass 2 of
//! the former `writeback_field_load_locals_with_table`). The LHS type check
//! in typeck's `infer_rvalue` Shl/Shr arm (Stage 18.412) makes it redundant.
//! Per §1.0 原則 5 (去除兼容思维): unused code removed.

use crate::mir::ty::*;

/// Whether a type can be used in arithmetic ops (Add/Sub/Mul/Div/Rem).
///
/// G7 fix (Stage 2.4f): Bool, Str, Tuple, Array, etc. are NOT arithmetic.
/// Int, Uint, Float, and Infer (deferred) are.
pub(super) fn is_arithmetic_ty(ty: &Ty) -> bool {
    matches!(
        &ty.kind,
        TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_) | TyKind::Infer(_) | TyKind::Error
    )
}

/// Whether a type can be negated with unary `-`.
///
/// Int, Uint, Float, Infer, Error are negatable. Bool, Str, Tuple are not.
pub(super) fn is_negatable_ty(ty: &Ty) -> bool {
    is_arithmetic_ty(ty)
}

/// Whether a type can be used with `!` (bitwise NOT).
///
/// Bool, Int, Uint, IntVar, TyVar, Error are notable.
/// Float, FloatVar, Str, Tuple are NOT notable.
///
/// G8 fix (Stage 2.4g): `!3.14` should error. FloatVar is excluded
/// because it can only resolve to Float (which is not notable).
pub(super) fn is_notable_ty(ty: &Ty) -> bool {
    matches!(
        &ty.kind,
        TyKind::Bool
            | TyKind::Int(_)
            | TyKind::Uint(_)
            | TyKind::Infer(InferVar::TyVar(_))
            | TyKind::Infer(InferVar::IntVar(_))
            | TyKind::Error
    )
}

/// Whether a type can be used as a shift count (rhs of Shl/Shr).
///
/// Int, Uint, Infer, Error are valid. Bool, Float, Str are not.
pub(super) fn is_shift_count_ty(ty: &Ty) -> bool {
    matches!(
        &ty.kind,
        TyKind::Int(_) | TyKind::Uint(_) | TyKind::Infer(_) | TyKind::Error
    )
}

/// Stage 3.58: Check if `rvalue_ty` can be implicitly coerced to `place_ty`.
///
/// Coercion rules (matching Landin's lenient type system):
///   - Bool → Int/Uint: comparison results widen to integers (codegen emits zext)
///   - Narrower Int/Uint → Wider Int/Uint: e.g., u8 → i32 (codegen emits zext/sext)
///   - Int/Uint → Int/Uint of same width: e.g., u32 → i32 (bitcast, lossless)
///   - Infer → anything: inference variables unify with anything
///   - Error → anything: error types suppress further errors
///
/// Returns `Ok(())` if coercion is possible, `Err(())` if not.
pub(super) fn can_coerce(place_ty: &Ty, rvalue_ty: &Ty) -> bool {
    use crate::ast::{IntTy, UintTy};
    use crate::mir::ty::TyKind;
    match (&place_ty.kind, &rvalue_ty.kind) {
        // Infer/Error: always coercible
        (TyKind::Infer(_), _) | (_, TyKind::Infer(_)) => true,
        (TyKind::Error, _) | (_, TyKind::Error) => true,
        // Stage 41 (v0.5 — TD-SPECIAL-2): Never (`!`) coerces to any type.
        // This is the bottom type — a `!`-typed expression (e.g.,
        // `panic!(...)`, `unreachable!(...)`, `loop {}`) can unify with
        // any expected type because it never produces a value.
        // Per §1.0 原則 6 (通解 > 特解): one rule for all noreturn paths.
        // Per §12 (最优 > 最小): root-cause fix — `!` is bottom type.
        (TyKind::Never, _) | (_, TyKind::Never) => true,
        // Stage 18.71: REMOVED `(Int(_)|Uint(_), Bool) => true` rule.
        // Per §1.0 原则 9 "正确 > 妥协": Rust does NOT allow implicit
        // Bool→Int/Uint coercion. `let x: i32 = true;` is a type error
        // in Rust, and so it must be in Landin. The previous rule was
        // overly permissive and masked 36 Stage 0 limitation test cases
        // (e2e-err-002, snd-*-type-mismatch, etc.).
        // Narrower int → wider int (e.g., i8 → i32, i16 → i64)
        (TyKind::Int(IntTy::I128), TyKind::Int(_)) => true,
        (TyKind::Int(IntTy::I64), TyKind::Int(IntTy::I8 | IntTy::I16 | IntTy::I32)) => true,
        (TyKind::Int(IntTy::I32), TyKind::Int(IntTy::I8 | IntTy::I16)) => true,
        (TyKind::Int(IntTy::I16), TyKind::Int(IntTy::I8)) => true,
        // Narrower uint → wider uint
        (TyKind::Uint(UintTy::U128), TyKind::Uint(_)) => true,
        (TyKind::Uint(UintTy::U64), TyKind::Uint(UintTy::U8 | UintTy::U16 | UintTy::U32)) => true,
        (TyKind::Uint(UintTy::U32), TyKind::Uint(UintTy::U8 | UintTy::U16)) => true,
        (TyKind::Uint(UintTy::U16), TyKind::Uint(UintTy::U8)) => true,
        // Int ↔ Uint of same width (e.g., i32 ↔ u32): lossless reinterpretation
        (TyKind::Int(IntTy::I8), TyKind::Uint(UintTy::U8)) => true,
        (TyKind::Int(IntTy::I16), TyKind::Uint(UintTy::U16)) => true,
        (TyKind::Int(IntTy::I32), TyKind::Uint(UintTy::U32)) => true,
        (TyKind::Int(IntTy::I64), TyKind::Uint(UintTy::U64)) => true,
        (TyKind::Int(IntTy::I128), TyKind::Uint(UintTy::U128)) => true,
        (TyKind::Uint(UintTy::U8), TyKind::Int(IntTy::I8)) => true,
        (TyKind::Uint(UintTy::U16), TyKind::Int(IntTy::I16)) => true,
        (TyKind::Uint(UintTy::U32), TyKind::Int(IntTy::I32)) => true,
        (TyKind::Uint(UintTy::U64), TyKind::Int(IntTy::I64)) => true,
        (TyKind::Uint(UintTy::U128), TyKind::Int(IntTy::I128)) => true,
        // Stage 3.59: Uint → wider Int (only widening, NOT narrowing).
        // Was: `(TyKind::Int(_), TyKind::Uint(_)) => true` which accepted
        // lossy narrowings like `i8 ← u64`. Now: explicit widening arms.
        (TyKind::Int(IntTy::I16), TyKind::Uint(UintTy::U8)) => true,
        (TyKind::Int(IntTy::I32), TyKind::Uint(UintTy::U8 | UintTy::U16)) => true,
        (TyKind::Int(IntTy::I64), TyKind::Uint(UintTy::U8 | UintTy::U16 | UintTy::U32)) => true,
        (
            TyKind::Int(IntTy::I128),
            TyKind::Uint(UintTy::U8 | UintTy::U16 | UintTy::U32 | UintTy::U64),
        ) => true,
        // Stage 3.59: f32 → f64 widening (lossless)
        (TyKind::Float(crate::ast::FloatTy::F64), TyKind::Float(crate::ast::FloatTy::F32)) => true,
        // Stage 14.74: &mut T → &T coercion (reborrow as immutable).
        (
            TyKind::Ref(_, crate::mir::ty::Mutability::Immutable, inner_a),
            TyKind::Ref(_, crate::mir::ty::Mutability::Mutable, inner_b),
        ) => inner_a == inner_b,
        // Same type: no coercion needed
        _ if place_ty.kind == rvalue_ty.kind => true,
        // Stage 18.71: Str ↔ Ref(_, _, Str) — fat pointer vs thin str.
        // The MIR lower may produce TyKind::Str for string literals but
        // the local decl may be Ref(_, _, Str). Allow this coercion.
        (TyKind::Str, TyKind::Ref(_, _, inner)) if matches!(inner.kind, TyKind::Str) => true,
        (TyKind::Ref(_, _, inner), TyKind::Str) if matches!(inner.kind, TyKind::Str) => true,
        // Stage 18.98: Adt with same DefId — only coercible if substs match
        // (recursively) OR one side has empty substs (inference case).
        // Per §2.0 原则 9 "正确 > 妥协": Vec<i32> != Vec<bool> (soundness).
        // The old code (`if a_def == b_def => true`) ignored substs entirely,
        // which made `can_coerce` return true for `Vec<i32> ↔ Vec<bool>`,
        // short-circuiting the `types_match_loose` check in check_statement.
        //
        // Empty substs (inference case) still coerce — they represent
        // "unknown, to be inferred" per unify.rs's empty-substs fallback.
        (TyKind::Adt(a_def, a_substs), TyKind::Adt(b_def, b_substs)) => {
            if a_def != b_def {
                return false;
            }
            // Empty substs = inference case → coercible
            if a_substs.is_empty() || b_substs.is_empty() {
                return true;
            }
            // Both have substs — must match in length and element-wise
            if a_substs.len() != b_substs.len() {
                return false;
            }
            a_substs
                .iter()
                .zip(b_substs.iter())
                .all(|(at, bt)| at.kind == bt.kind)
        }
        // Everything else: not coercible
        _ => false,
    }
}
