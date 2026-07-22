//! Stage 5.3: ty_is_copy_with_resolver tests
//!
//! Tests that ty_is_copy_with_resolver correctly handles primitives
//! and Adt types (with fallback for now — full Copy detection deferred).

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

#[test]
fn test_adt_fallback_copy() {
    let adt_ty = Ty::new(
        TyKind::Adt(landin_compiler::hir::DefId(0), vec![]),
        Span::DUMMY,
    );
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    assert!(ty_is_copy(&adt_ty), "Adt should be Copy (fallback)");
    assert!(
        ty_is_copy_with_resolver(&adt_ty, &resolver, &interner),
        "Adt should be Copy (resolver fallback — full detection deferred to Stage 5.4)"
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
