//! Stage 5.93: stdlib_trait_method_return_kind + stdlib_trait_method_param_kinds tests
//!
//! Tests the two new convenience accessor functions:
//! - `stdlib_trait_method_return_kind(trait, method) -> Option<StdlibTypeKind>`
//! - `stdlib_trait_method_param_kinds(trait, method) -> Option<&'static [StdlibTypeKind]>`
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{
    find_stdlib_trait_method, stdlib_trait_method_param_kinds, stdlib_trait_method_return_kind,
    StdlibTypeKind,
};

// ============================================================
// stdlib_trait_method_return_kind tests
// ============================================================

/// Drop::drop return_kind is Unit.
#[test]
fn test_return_kind_drop_unit() {
    let kind = stdlib_trait_method_return_kind("Drop", "drop");
    assert_eq!(kind, Some(StdlibTypeKind::Unit));
}

/// Clone::clone return_kind is AllocType (Self).
#[test]
fn test_return_kind_clone_alloc_type() {
    let kind = stdlib_trait_method_return_kind("Clone", "clone");
    assert_eq!(kind, Some(StdlibTypeKind::AllocType));
}

/// Display::fmt return_kind is StdType (Result<(), Error>).
#[test]
fn test_return_kind_display_std_type() {
    let kind = stdlib_trait_method_return_kind("Display", "fmt");
    assert_eq!(kind, Some(StdlibTypeKind::StdType));
}

/// PartialEq::eq return_kind is Bool.
#[test]
fn test_return_kind_partial_eq_bool() {
    let kind = stdlib_trait_method_return_kind("PartialEq", "eq");
    assert_eq!(kind, Some(StdlibTypeKind::Bool));
}

/// Foo::bar return_kind is None (not in stdlib).
#[test]
fn test_return_kind_foo_none() {
    let kind = stdlib_trait_method_return_kind("Foo", "bar");
    assert_eq!(kind, None);
}

/// Drop::nonexistent return_kind is None (method not found).
#[test]
fn test_return_kind_nonexistent_method_none() {
    let kind = stdlib_trait_method_return_kind("Drop", "nonexistent");
    assert_eq!(kind, None);
}

// ============================================================
// stdlib_trait_method_param_kinds tests
// ============================================================

/// Drop::drop param_kinds is empty (no params).
#[test]
fn test_param_kinds_drop_empty() {
    let kinds = stdlib_trait_method_param_kinds("Drop", "drop");
    assert!(kinds.is_some());
    assert!(kinds.unwrap().is_empty());
}

/// Display::fmt param_kinds is [StdType] (Formatter).
#[test]
fn test_param_kinds_display_fmt_std_type() {
    let kinds = stdlib_trait_method_param_kinds("Display", "fmt");
    assert!(kinds.is_some());
    let kinds = kinds.unwrap();
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0], StdlibTypeKind::StdType);
}

/// Clone::clone_from param_kinds is [AllocType] (&Self).
#[test]
fn test_param_kinds_clone_from_alloc_type() {
    let kinds = stdlib_trait_method_param_kinds("Clone", "clone_from");
    assert!(kinds.is_some());
    let kinds = kinds.unwrap();
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0], StdlibTypeKind::AllocType);
}

/// Foo::bar param_kinds is None (not in stdlib).
#[test]
fn test_param_kinds_foo_none() {
    let kinds = stdlib_trait_method_param_kinds("Foo", "bar");
    assert_eq!(kinds, None);
}

// ============================================================
// Consistency with find_stdlib_trait_method tests
// ============================================================

/// stdlib_trait_method_return_kind matches find_stdlib_trait_method().return_kind.
#[test]
fn test_return_kind_consistent_with_find() {
    let traits_and_methods = [
        ("Clone", "clone"),
        ("Clone", "clone_from"),
        ("Drop", "drop"),
        ("Default", "default"),
        ("Display", "fmt"),
        ("Debug", "fmt"),
        ("PartialEq", "eq"),
        ("PartialEq", "ne"),
        ("PartialOrd", "partial_cmp"),
        ("Ord", "cmp"),
        ("Hash", "hash"),
    ];
    for (trait_name, method_name) in &traits_and_methods {
        let via_accessor = stdlib_trait_method_return_kind(trait_name, method_name);
        let via_find = find_stdlib_trait_method(trait_name, method_name).map(|m| m.return_kind);
        assert_eq!(
            via_accessor, via_find,
            "mismatch for {}.{}: accessor={:?}, find={:?}",
            trait_name, method_name, via_accessor, via_find
        );
    }
}

/// stdlib_trait_method_param_kinds matches find_stdlib_trait_method().param_kinds.
#[test]
fn test_param_kinds_consistent_with_find() {
    let traits_and_methods = [
        ("Clone", "clone"),
        ("Clone", "clone_from"),
        ("Drop", "drop"),
        ("Display", "fmt"),
        ("Debug", "fmt"),
        ("Hash", "hash"),
        ("PartialEq", "eq"),
        ("PartialOrd", "partial_cmp"),
    ];
    for (trait_name, method_name) in &traits_and_methods {
        let via_accessor = stdlib_trait_method_param_kinds(trait_name, method_name);
        let via_find = find_stdlib_trait_method(trait_name, method_name).map(|m| m.param_kinds);
        assert_eq!(
            via_accessor, via_find,
            "mismatch for {}.{}: accessor={:?}, find={:?}",
            trait_name, method_name, via_accessor, via_find
        );
    }
}
