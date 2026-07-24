//! Stage 5.96: stdlib_trait_methods_by_return_kind tests
//!
//! Tests `stdlib_trait_methods_by_return_kind()` — reverse query returning all
//! stdlib trait methods with a given return type kind.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{
    stdlib_all_traits, stdlib_trait_method_return_kind, stdlib_trait_methods,
    stdlib_trait_methods_by_return_kind, StdlibTypeKind,
};

// ============================================================
// Non-empty tests for common return kinds
// ============================================================

/// Unit returns non-empty (Drop::drop, clone_from, assign ops).
#[test]
fn test_by_return_kind_unit_non_empty() {
    let methods = stdlib_trait_methods_by_return_kind(StdlibTypeKind::Unit);
    assert!(!methods.is_empty());
}

/// Bool returns non-empty (PartialEq::eq/ne).
#[test]
fn test_by_return_kind_bool_non_empty() {
    let methods = stdlib_trait_methods_by_return_kind(StdlibTypeKind::Bool);
    assert!(!methods.is_empty());
}

/// AllocType returns non-empty (Clone::clone, Default::default, arithmetic ops).
#[test]
fn test_by_return_kind_alloc_type_non_empty() {
    let methods = stdlib_trait_methods_by_return_kind(StdlibTypeKind::AllocType);
    assert!(!methods.is_empty());
}

/// StdType returns non-empty (Display::fmt, PartialOrd::partial_cmp, etc.).
#[test]
fn test_by_return_kind_std_type_non_empty() {
    let methods = stdlib_trait_methods_by_return_kind(StdlibTypeKind::StdType);
    assert!(!methods.is_empty());
}

// ============================================================
// Contains tests
// ============================================================

/// Unit contains ("Drop", "drop").
#[test]
fn test_by_return_kind_unit_contains_drop() {
    let methods = stdlib_trait_methods_by_return_kind(StdlibTypeKind::Unit);
    assert!(
        methods.contains(&("Drop", "drop")),
        "expected (Drop, drop) in Unit return methods"
    );
}

/// Bool contains ("PartialEq", "eq").
#[test]
fn test_by_return_kind_bool_contains_eq() {
    let methods = stdlib_trait_methods_by_return_kind(StdlibTypeKind::Bool);
    assert!(
        methods.contains(&("PartialEq", "eq")),
        "expected (PartialEq, eq) in Bool return methods"
    );
}

// ============================================================
// Consistency tests
// ============================================================

/// All returned methods have return_kind matching the query parameter.
#[test]
fn test_by_return_kind_all_match() {
    let kinds = [
        StdlibTypeKind::Unit,
        StdlibTypeKind::Bool,
        StdlibTypeKind::AllocType,
        StdlibTypeKind::StdType,
    ];
    for kind in &kinds {
        let methods = stdlib_trait_methods_by_return_kind(*kind);
        for &(trait_name, method_name) in &methods {
            let actual_kind = stdlib_trait_method_return_kind(trait_name, method_name)
                .expect("method should exist");
            assert_eq!(
                actual_kind, *kind,
                "method {}.{} has return_kind {:?} but was returned for {:?}",
                trait_name, method_name, actual_kind, kind
            );
        }
    }
}

/// All return_kinds together cover all non-marker methods.
#[test]
fn test_by_return_kind_all_kinds_cover_all_methods() {
    let all_traits = stdlib_all_traits();
    let mut expected = 0;
    for trait_name in &all_traits {
        if let Some(methods) = stdlib_trait_methods(trait_name) {
            expected += methods.len();
        }
    }

    // Sum across all StdlibTypeKind variants that appear in the registry
    let mut total = 0;
    for kind in [
        StdlibTypeKind::Unit,
        StdlibTypeKind::Bool,
        StdlibTypeKind::AllocType,
        StdlibTypeKind::StdType,
    ] {
        total += stdlib_trait_methods_by_return_kind(kind).len();
    }
    assert_eq!(
        total, expected,
        "all return_kinds ({}) should cover all methods ({})",
        total, expected
    );
}

// ============================================================
// Robustness tests
// ============================================================

/// No side effects — repeated calls return same result.
#[test]
fn test_by_return_kind_no_side_effects() {
    let a1 = stdlib_trait_methods_by_return_kind(StdlibTypeKind::Unit);
    let a2 = stdlib_trait_methods_by_return_kind(StdlibTypeKind::Unit);
    assert_eq!(a1, a2);
}

/// I32 returns empty (no stdlib trait method returns I32 directly).
#[test]
fn test_by_return_kind_i32_empty() {
    let methods = stdlib_trait_methods_by_return_kind(StdlibTypeKind::I32);
    assert!(
        methods.is_empty(),
        "expected no methods returning I32, got {:?}",
        methods
    );
}
