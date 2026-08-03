//! Stage 5.12: Copy detection unification tests
//!
//! Tests that `ty_is_copy_unified()` (and the refactored
//! `ty_is_copy_with_resolver`) correctly delegate primitive Copy checks
//! to `is_primitive_copy_kind()` while still handling Tuple/Array/Adt
//! via recursion/resolver.
//!
//! Per §16: tests use the public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::borrowck::{ty_is_copy_unified, ty_is_copy_with_resolver};
use landin_compiler::compile;
use landin_compiler::mir::ty::{Ty, TyKind};
use landin_compiler::session::Span;
use landin_compiler::traits::TraitResolver;
use lasso::Rodeo;

/// `ty_is_copy_unified` should return true for primitive types (i32).
#[test]
fn test_unified_primitive_is_copy() {
    let i32_ty = Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY);
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    assert!(
        ty_is_copy_unified(&i32_ty, &resolver, &interner),
        "i32 should be Copy via ty_is_copy_unified"
    );
}

/// `ty_is_copy_unified` should agree with `ty_is_copy_with_resolver` for
/// all type kinds (they're now the same function internally).
#[test]
fn test_unified_matches_with_resolver() {
    let cases = [
        Ty::new(TyKind::Bool, Span::DUMMY),
        Ty::new(TyKind::Char, Span::DUMMY),
        Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY),
        Ty::new(TyKind::Never, Span::DUMMY),
        Ty::new(TyKind::Str, Span::DUMMY),
    ];
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    for ty in &cases {
        let unified = ty_is_copy_unified(ty, &resolver, &interner);
        let with_resolver = ty_is_copy_with_resolver(ty, &resolver, &interner);
        assert_eq!(
            unified, with_resolver,
            "ty_is_copy_unified and ty_is_copy_with_resolver should agree for {:?}",
            ty.kind
        );
    }
}

/// Adt without `impl Copy` should return false via unified check.
#[test]
fn test_unified_adt_without_copy_not_copy() {
    let adt_ty = Ty::new(
        TyKind::Adt(landin_compiler::hir::DefId(0), vec![].into()),
        Span::DUMMY,
    );
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    assert!(
        !ty_is_copy_unified(&adt_ty, &resolver, &interner),
        "Adt without impl Copy should NOT be Copy (unified)"
    );
}

/// Integration test: compile a program with `impl Copy for S` and verify
/// the resolver + unified check work end-to-end.
#[test]
fn test_unified_integration_with_impl_copy() {
    let result = compile("struct S; impl Copy for S {} fn main() {}");
    // Find S's DefId
    let s_spur = result.interner.get("S").expect("S interned");
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .expect("S DefId found");

    let s_ty = Ty::new(TyKind::Adt(s_def_id, vec![].into()), Span::DUMMY);
    assert!(
        ty_is_copy_unified(&s_ty, &result.trait_resolver, &result.interner),
        "S with impl Copy should be Copy via unified check"
    );
}

/// Integration test: struct with `impl Drop` should NOT be Copy.
/// Stage 16.06: Unit structs (no fields) are DERIVED Copy. To test the
/// "not Copy" path, use `impl Drop` (Copy+Drop conflict).
#[test]
fn test_unified_integration_without_impl_copy() {
    let result = compile("struct S; impl Drop for S { fn drop(self: &mut S) {} } fn main() {}");
    let s_spur = result.interner.get("S").expect("S interned");
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .expect("S DefId found");

    let s_ty = Ty::new(TyKind::Adt(s_def_id, vec![].into()), Span::DUMMY);
    assert!(
        !ty_is_copy_unified(&s_ty, &result.trait_resolver, &result.interner),
        "S with impl Drop should NOT be Copy via unified check (Copy+Drop conflict, Stage 16.06)"
    );
}
