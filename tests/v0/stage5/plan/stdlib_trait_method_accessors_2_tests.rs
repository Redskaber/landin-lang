//! Stage 5.94: stdlib_trait_method remaining field accessors tests
//!
//! Tests the 3 new convenience accessor functions:
//! - `stdlib_trait_method_self_kind(trait, method) -> Option<StdlibSelfKind>`
//! - `stdlib_trait_method_param_count(trait, method) -> Option<u32>`
//! - `stdlib_trait_method_is_unsafe(trait, method) -> Option<bool>`
//!
//! Completes full field accessor coverage for StdlibTraitMethod
//! (name is a query param, self_kind/param_count/return_kind/param_kinds/is_unsafe all have accessors).
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{
    find_stdlib_trait_method, stdlib_trait_method_is_unsafe, stdlib_trait_method_param_count,
    stdlib_trait_method_self_kind, StdlibSelfKind,
};

// ============================================================
// stdlib_trait_method_self_kind tests
// ============================================================

/// Clone::clone self_kind is SelfByRef.
#[test]
fn test_self_kind_clone_self_by_ref() {
    let kind = stdlib_trait_method_self_kind("Clone", "clone");
    assert_eq!(kind, Some(StdlibSelfKind::SelfByRef));
}

/// Drop::drop self_kind is SelfByMutRef.
#[test]
fn test_self_kind_drop_self_by_mut_ref() {
    let kind = stdlib_trait_method_self_kind("Drop", "drop");
    assert_eq!(kind, Some(StdlibSelfKind::SelfByMutRef));
}

/// Default::default self_kind is NoSelf (associated function).
#[test]
fn test_self_kind_default_no_self() {
    let kind = stdlib_trait_method_self_kind("Default", "default");
    assert_eq!(kind, Some(StdlibSelfKind::NoSelf));
}

/// Foo::bar self_kind is None (not in stdlib).
#[test]
fn test_self_kind_foo_none() {
    let kind = stdlib_trait_method_self_kind("Foo", "bar");
    assert_eq!(kind, None);
}

// ============================================================
// stdlib_trait_method_param_count tests
// ============================================================

/// Drop::drop param_count is 0 (no params).
#[test]
fn test_param_count_drop_zero() {
    let count = stdlib_trait_method_param_count("Drop", "drop");
    assert_eq!(count, Some(0));
}

/// Display::fmt param_count is 1 (Formatter).
#[test]
fn test_param_count_display_one() {
    let count = stdlib_trait_method_param_count("Display", "fmt");
    assert_eq!(count, Some(1));
}

/// Clone::clone param_count is 0 (no params).
#[test]
fn test_param_count_clone_zero() {
    let count = stdlib_trait_method_param_count("Clone", "clone");
    assert_eq!(count, Some(0));
}

/// Foo::bar param_count is None (not in stdlib).
#[test]
fn test_param_count_foo_none() {
    let count = stdlib_trait_method_param_count("Foo", "bar");
    assert_eq!(count, None);
}

// ============================================================
// stdlib_trait_method_is_unsafe tests
// ============================================================

/// Drop::drop is_unsafe is false.
#[test]
fn test_is_unsafe_drop_false() {
    let is_unsafe = stdlib_trait_method_is_unsafe("Drop", "drop");
    assert_eq!(is_unsafe, Some(false));
}

/// Clone::clone is_unsafe is false.
#[test]
fn test_is_unsafe_clone_false() {
    let is_unsafe = stdlib_trait_method_is_unsafe("Clone", "clone");
    assert_eq!(is_unsafe, Some(false));
}

/// Foo::bar is_unsafe is None (not in stdlib).
#[test]
fn test_is_unsafe_foo_none() {
    let is_unsafe = stdlib_trait_method_is_unsafe("Foo", "bar");
    assert_eq!(is_unsafe, None);
}

// ============================================================
// Consistency with find_stdlib_trait_method tests
// ============================================================

/// stdlib_trait_method_self_kind matches find_stdlib_trait_method().self_kind.
#[test]
fn test_self_kind_consistent_with_find() {
    let pairs = [
        ("Clone", "clone"),
        ("Clone", "clone_from"),
        ("Drop", "drop"),
        ("Default", "default"),
        ("Display", "fmt"),
        ("PartialEq", "eq"),
        ("PartialEq", "ne"),
        ("PartialOrd", "partial_cmp"),
        ("Ord", "cmp"),
        ("Hash", "hash"),
    ];
    for (trait_name, method_name) in &pairs {
        let via_accessor = stdlib_trait_method_self_kind(trait_name, method_name);
        let via_find = find_stdlib_trait_method(trait_name, method_name).map(|m| m.self_kind);
        assert_eq!(
            via_accessor, via_find,
            "mismatch for {}.{}: accessor={:?}, find={:?}",
            trait_name, method_name, via_accessor, via_find
        );
    }
}

/// stdlib_trait_method_param_count matches find_stdlib_trait_method().param_count.
#[test]
fn test_param_count_consistent_with_find() {
    let pairs = [
        ("Clone", "clone"),
        ("Clone", "clone_from"),
        ("Drop", "drop"),
        ("Display", "fmt"),
        ("PartialEq", "eq"),
        ("Hash", "hash"),
    ];
    for (trait_name, method_name) in &pairs {
        let via_accessor = stdlib_trait_method_param_count(trait_name, method_name);
        let via_find = find_stdlib_trait_method(trait_name, method_name).map(|m| m.param_count);
        assert_eq!(
            via_accessor, via_find,
            "mismatch for {}.{}: accessor={:?}, find={:?}",
            trait_name, method_name, via_accessor, via_find
        );
    }
}

/// stdlib_trait_method_is_unsafe matches find_stdlib_trait_method().is_unsafe.
#[test]
fn test_is_unsafe_consistent_with_find() {
    let pairs = [
        ("Clone", "clone"),
        ("Drop", "drop"),
        ("Default", "default"),
        ("Display", "fmt"),
        ("PartialEq", "eq"),
        ("Hash", "hash"),
        ("Add", "add"),
        ("Neg", "neg"),
    ];
    for (trait_name, method_name) in &pairs {
        let via_accessor = stdlib_trait_method_is_unsafe(trait_name, method_name);
        let via_find = find_stdlib_trait_method(trait_name, method_name).map(|m| m.is_unsafe);
        assert_eq!(
            via_accessor, via_find,
            "mismatch for {}.{}: accessor={:?}, find={:?}",
            trait_name, method_name, via_accessor, via_find
        );
    }
}
