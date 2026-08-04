//! Stage 6.14 (TD-024): Copy semantics — Copy trait detection.
//!
//! Per 04-ownership-borrowing.md §4.5 (Move tracking) — Copy vs Move
//! semantics is determined by whether a type implements `Copy`.
//! Extracted from `mod.rs` per `docs/stage-committee-process.md` v3.21
//! §14.4 + §13.4.
//!
//! Owns 3 functions:
//! - `ty_is_copy` (MVP Copy detection without TraitResolver)
//! - `ty_is_copy_with_resolver` (precise Copy detection via TraitResolver)
//! - `ty_is_copy_unified` (unified entry point, delegates to _with_resolver)

/// Determine whether a type implements `Copy`.
///
/// Per Landin semantics (mirroring Rust), the following types are Copy:
/// - Primitives: bool, char, int, uint, float
/// - References: `&T` (shared refs are always Copy; `&mut T` is not Copy
///   but Move semantics are checked elsewhere)
/// - Raw pointers: `*const T`, `*mut T`
/// - Function definitions and function pointers
/// - Tuples whose every element is Copy
/// - Arrays of Copy types (size is part of the type)
/// - Slices are NOT Copy (they're unsized)
/// - The unit type `()` is Copy
///
/// ADTs (struct/enum) require an explicit `#[derive(Copy)]` annotation;
/// for Stage 2.4c we conservatively treat all Adt types as non-Copy
/// (the TraitResolver, which would consult the derive list, is Stage 3).
/// This is the safe default — a false negative (saying "not Copy" when
/// it actually is) just produces a spurious error; a false positive
/// (saying "Copy" when it isn't) would be unsound.
///
/// `Infer` and `Error` are treated as Copy to avoid spurious errors
/// during type inference (the type isn't known yet, so we give the
/// benefit of the doubt).
///
/// Stage 16.06: DEPRECATED. This function is UNSOUND — it returns `true`
/// for ALL Adt types, which is incorrect for types with `impl Drop` or
/// non-Copy fields. The driver now uses `ty_is_copy_with_resolver` (via
/// `BorrowChecker::with_resolver_and_sigs`) which is sound. This function
/// remains only for test contexts that construct `BorrowChecker::new()`
/// without a resolver. New code should use `ty_is_copy_with_resolver` or
/// `ty_is_copy_unified`.
///
/// Per §23.6: deprecated with note pointing to the §16-compliant
/// alternative.
#[deprecated(
    note = "Unsound: returns true for ALL Adt types. Use ty_is_copy_with_resolver (via BorrowChecker::with_resolver_and_sigs) or ty_is_copy_unified instead. (Stage 16.06)"
)]
pub fn ty_is_copy(ty: &crate::mir::ty::Ty) -> bool {
    use crate::mir::ty::TyKind::*;
    match &ty.kind {
        Bool | Char | Int(_) | Uint(_) | Float(_) => true,
        // Shared refs are Copy; mut refs are not. We treat all refs as
        // Copy here for simplicity — the mut-ref case is rare and the
        // worst case is a spurious acceptance, which is caught later by
        // the move tracker (the second use of a moved &mut would fail).
        Ref(_, _, _) => true,
        RawPtr(_, _) => true,
        FnDef(_, _) | FnPtr(_) => true,
        Never => true,
        #[allow(deprecated)]
        Tuple(tys) => tys.iter().all(ty_is_copy),
        #[allow(deprecated)]
        Array(inner, _) => ty_is_copy(inner),
        // Infer and Error: assume Copy to avoid spurious errors.
        Infer(_) | Error | Foreign => true,
        // Stage 5.3: Treat Adt (struct/enum) as Copy by default (fallback).
        // Use `ty_is_copy_with_resolver` for precise Copy detection.
        Adt(_, _) => true,
        Str | Slice(_) | Closure(_, _) | Param(_) => false,
    }
}

/// Stage 5.4: Check if a type is Copy using TraitResolver.
///
/// Now fully active for Adt types — uses `type_by_def_id` reverse map
/// to look up the type name from its DefId, then checks if `impl Copy`
/// exists for that type via `resolver.is_copy()`. If no Copy impl is
/// found, the type is NOT Copy.
///
/// For non-Adt types, behavior is identical to `ty_is_copy`.
///
/// Stage 5.12: Primitive branches now delegate to `is_primitive_copy_kind()`
/// for consistency — single source of truth for which TyKinds are always Copy.
/// The match still handles Tuple/Array (recursive) and Adt (resolver query)
/// which `is_primitive_copy_kind` cannot (it's string-based, no recursion).
pub fn ty_is_copy_with_resolver(
    ty: &crate::mir::ty::Ty,
    resolver: &crate::traits::TraitResolver,
    interner: &lasso::Rodeo,
) -> bool {
    use crate::mir::ty::TyKind::*;
    match &ty.kind {
        // Stage 5.12: delegate primitive Copy check to the unified helper.
        // is_primitive_copy_kind returns true for Bool/Char/Int/Uint/Float/
        // Never/Ref/RawPtr/FnDef/FnPtr — matching the old hardcoded branches.
        Bool
        | Char
        | Int(_)
        | Uint(_)
        | Float(_)
        | Ref(_, _, _)
        | RawPtr(_, _)
        | FnDef(_, _)
        | FnPtr(_)
        | Never => crate::traits::is_primitive_copy_kind(&format!("{:?}", ty.kind)),
        Tuple(tys) => tys
            .iter()
            .all(|t| ty_is_copy_with_resolver(t, resolver, interner)),
        Array(inner, _) => ty_is_copy_with_resolver(inner, resolver, interner),
        Infer(_) | Error | Foreign => true,
        // Stage 5.9: Use TraitResolver to check for Copy impl via the
        // builtin Copy trait. Stage 5.8 ensures "Copy" is always interned,
        // and Stage 5.9's is_copy_builtin() handles the lookup cleanly.
        // Old fallback of `true` (treating all Adt as Copy) was unsound —
        // now correctly returns false if no `impl Copy for <Type>` exists.
        Adt(def_id, _) => resolver.is_copy_builtin(*def_id, interner),
        // Stage 16.22: Closures with no captures (empty substs) are Copy.
        // This allows chained calls like f(f(f(0))) where f is a no-capture
        // closure.
        //
        // Stage 16.29 (通解 — field-level Copy derivation for closures):
        // Closures with ALL-Copy captures are also Copy. This mirrors
        // Rust's `#[derive(Copy)]` for closure structs — if every field
        // (capture) is Copy, the closure struct is Copy.
        //
        // This fixes borrowck false positives for `f()()` patterns where
        // f returns a closure with i32 captures. Without this, borrowck
        // flags `Operand::Move` on the returned closure as "use of moved
        // value: {closure} does not implement Copy".
        //
        // Per §1.0 原則 6 "通用 > 特例": one rule for all closures —
        // Copy iff all captures are Copy (including zero captures).
        // Per §1.0 原則 9 "正确 > 妥协": match Rust's semantics for
        // closure Copy derivation.
        Closure(_, substs) => substs
            .iter()
            .all(|t| ty_is_copy_with_resolver(t, resolver, interner)),
        Str | Slice(_) | Param(_) => false,
    }
}

/// Stage 5.12: Unified Copy check — single entry point that combines
/// primitive Copy detection (via `is_primitive_copy_kind`) with resolver-
/// based Copy detection (via `is_copy_builtin`).
///
/// This is the preferred Copy-detection entry point for new code. It
/// delegates to `ty_is_copy_with_resolver` (which now uses
/// `is_primitive_copy_kind` for primitives) and is kept as a separate
/// name to make the "unified" intent explicit.
///
/// Per API-naming-standard §3: `ty_is_copy_` prefix consistent with
/// `ty_is_copy` / `ty_is_copy_with_resolver`; `_unified` suffix distinguishes.
pub fn ty_is_copy_unified(
    ty: &crate::mir::ty::Ty,
    resolver: &crate::traits::TraitResolver,
    interner: &lasso::Rodeo,
) -> bool {
    ty_is_copy_with_resolver(ty, resolver, interner)
}
