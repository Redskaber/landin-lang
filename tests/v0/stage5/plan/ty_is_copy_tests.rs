//! Stage 5.3: ty_is_copy_with_resolver tests
//!
//! Tests that ty_is_copy_with_resolver correctly handles primitives
//! and Adt types.
//!
//! Stage 5.9 update: the old `test_adt_fallback_copy` asserted the unsound
//! fallback behavior (Adt treated as Copy when no `impl Copy` exists).
//! Stage 5.9 fixed this — Adt without `impl Copy` is now correctly NOT
//! Copy. The test has been updated to reflect the sound behavior.
//!
//! Stage 16.06: `ty_is_copy` is deprecated (unsound). These tests use it
//! only to verify the fallback path (test contexts without resolver).

#![allow(deprecated)] // Stage 16.06: ty_is_copy is deprecated, tests verify fallback

use landin_compiler::borrowck::{ty_is_copy, ty_is_copy_with_resolver};
use landin_compiler::mir::ty::{Ty, TyKind};
use landin_compiler::session::Span;
use landin_compiler::traits::TraitResolver;
use lasso::Rodeo;

#[test]
fn test_primitives_always_copy() {
    let i32_ty = Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY);
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    assert!(ty_is_copy(&i32_ty), "i32 should be Copy (fallback)");
    assert!(
        ty_is_copy_with_resolver(&i32_ty, &resolver, &interner),
        "i32 should be Copy (with resolver)"
    );
}

/// Stage 5.9: Adt without `impl Copy` should NOT be Copy.
///
/// The old test (Stage 5.3) asserted `true` because the resolver fell back
/// to `true` when "Copy" wasn't interned. Stage 5.9 fixes this to `false`
/// (sound) — only types with an explicit `impl Copy for <Type>` are Copy.
/// This test uses an empty TraitResolver (no builtin registration), so
/// `is_copy_builtin` returns `false` (defensive fallback).
#[test]
fn test_adt_without_copy_impl_not_copy() {
    let adt_ty = Ty::new(
        TyKind::Adt(landin_compiler::hir::DefId(0), vec![].into()),
        Span::DUMMY,
    );
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    // ty_is_copy (legacy, no resolver) still returns true for Adt — it
    // doesn't consult the resolver at all and treats all Adt as Copy.
    // This is the legacy behavior kept for backward compat.
    assert!(ty_is_copy(&adt_ty), "legacy ty_is_copy treats Adt as Copy");
    // ty_is_copy_with_resolver (Stage 5.9) correctly returns false —
    // no `impl Copy` exists for this DefId.
    assert!(
        !ty_is_copy_with_resolver(&adt_ty, &resolver, &interner),
        "Adt without impl Copy should NOT be Copy (Stage 5.9 soundness fix)"
    );
}

#[test]
fn test_str_not_copy() {
    let str_ty = Ty::new(TyKind::Str, Span::DUMMY);
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    assert!(!ty_is_copy(&str_ty), "str should NOT be Copy");
    assert!(
        !ty_is_copy_with_resolver(&str_ty, &resolver, &interner),
        "str should NOT be Copy (with resolver)"
    );
}
