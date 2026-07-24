//! Stage 5.85: is_stdlib_trait tests
//!
//! Tests `is_stdlib_trait()` — trait-level membership query. Checks if a
//! trait name is a stdlib trait (marker or with methods).
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{
    is_stdlib_marker_trait, is_stdlib_trait, is_stdlib_trait_method, stdlib_trait_methods,
};

// ============================================================
// Marker traits
// ============================================================

/// "Copy" is a stdlib marker trait.
#[test]
fn test_is_stdlib_trait_copy_marker() {
    assert!(is_stdlib_trait("Copy"));
}

/// "Send" is a stdlib marker trait.
#[test]
fn test_is_stdlib_trait_send_marker() {
    assert!(is_stdlib_trait("Send"));
}

/// "Sync" is a stdlib marker trait.
#[test]
fn test_is_stdlib_trait_sync_marker() {
    assert!(is_stdlib_trait("Sync"));
}

/// "Sized" is a stdlib marker trait.
#[test]
fn test_is_stdlib_trait_sized_marker() {
    assert!(is_stdlib_trait("Sized"));
}

/// "Unpin" is a stdlib marker trait.
#[test]
fn test_is_stdlib_trait_unpin_marker() {
    assert!(is_stdlib_trait("Unpin"));
}

/// "Eq" is a stdlib marker trait.
#[test]
fn test_is_stdlib_trait_eq_marker() {
    assert!(is_stdlib_trait("Eq"));
}

// ============================================================
// Traits with methods
// ============================================================

/// "Clone" is a stdlib trait with methods.
#[test]
fn test_is_stdlib_trait_clone() {
    assert!(is_stdlib_trait("Clone"));
}

/// "Drop" is a stdlib trait with methods.
#[test]
fn test_is_stdlib_trait_drop() {
    assert!(is_stdlib_trait("Drop"));
}

/// "Display" is a stdlib trait with methods.
#[test]
fn test_is_stdlib_trait_display() {
    assert!(is_stdlib_trait("Display"));
}

/// "Add" is a stdlib arithmetic trait.
#[test]
fn test_is_stdlib_trait_add() {
    assert!(is_stdlib_trait("Add"));
}

/// "Iterator" is a stdlib trait.
#[test]
fn test_is_stdlib_trait_iterator() {
    assert!(is_stdlib_trait("Iterator"));
}

/// "AddAssign" is a stdlib assign trait.
#[test]
fn test_is_stdlib_trait_add_assign() {
    assert!(is_stdlib_trait("AddAssign"));
}

// ============================================================
// Non-stdlib traits
// ============================================================

/// "Foo" is not a stdlib trait.
#[test]
fn test_is_stdlib_trait_foo_false() {
    assert!(!is_stdlib_trait("Foo"));
}

/// "Bar" is not a stdlib trait.
#[test]
fn test_is_stdlib_trait_bar_false() {
    assert!(!is_stdlib_trait("Bar"));
}

/// "MyTrait" is not a stdlib trait.
#[test]
fn test_is_stdlib_trait_my_trait_false() {
    assert!(!is_stdlib_trait("MyTrait"));
}

/// Empty string is not a stdlib trait.
#[test]
fn test_is_stdlib_trait_empty_false() {
    assert!(!is_stdlib_trait(""));
}

/// "clone" (method name) is not a stdlib trait (case-sensitive).
#[test]
fn test_is_stdlib_trait_lowercase_clone_false() {
    assert!(!is_stdlib_trait("clone"));
}

/// "drop" (method name) is not a stdlib trait (case-sensitive).
#[test]
fn test_is_stdlib_trait_lowercase_drop_false() {
    assert!(!is_stdlib_trait("drop"));
}

// ============================================================
// Consistency with existing queries
// ============================================================

/// All marker traits return true for is_stdlib_trait.
#[test]
fn test_is_stdlib_trait_consistent_with_marker_query() {
    let markers = ["Copy", "Send", "Sync", "Sized", "Unpin", "Eq"];
    for m in &markers {
        assert!(is_stdlib_marker_trait(m), "{} should be a marker trait", m);
        assert!(is_stdlib_trait(m), "{} should be a stdlib trait", m);
    }
}

/// All traits with methods return true for is_stdlib_trait.
#[test]
fn test_is_stdlib_trait_consistent_with_methods_query() {
    let method_traits = [
        "Clone",
        "Drop",
        "Default",
        "Display",
        "Debug",
        "PartialEq",
        "PartialOrd",
        "Ord",
        "Hash",
        "Add",
        "Sub",
        "Mul",
    ];
    for t in &method_traits {
        assert!(
            stdlib_trait_methods(t).is_some(),
            "{} should have methods",
            t
        );
        assert!(is_stdlib_trait(t), "{} should be a stdlib trait", t);
    }
}

/// is_stdlib_trait returns false when stdlib_trait_methods returns None.
#[test]
fn test_is_stdlib_trait_consistent_with_none() {
    let non_stdlib = ["Foo", "Bar", "Baz", "NonExistent", "MyTrait"];
    for t in &non_stdlib {
        assert!(
            stdlib_trait_methods(t).is_none(),
            "{} should not be in registry",
            t
        );
        assert!(!is_stdlib_trait(t), "{} should not be a stdlib trait", t);
    }
}

/// Marker traits: is_stdlib_trait true but is_stdlib_trait_method false (no methods).
#[test]
fn test_is_stdlib_trait_marker_vs_method_query() {
    // Copy is a marker trait — is_stdlib_trait true, but no methods
    assert!(is_stdlib_trait("Copy"));
    assert!(!is_stdlib_trait_method("Copy", "clone")); // Copy has no methods
}

/// Method traits: both is_stdlib_trait and is_stdlib_trait_method true.
#[test]
fn test_is_stdlib_trait_method_trait_both_true() {
    assert!(is_stdlib_trait("Clone"));
    assert!(is_stdlib_trait_method("Clone", "clone"));
}

/// No side effects — repeated calls return same result.
#[test]
fn test_is_stdlib_trait_no_side_effects() {
    let r1 = is_stdlib_trait("Clone");
    let r2 = is_stdlib_trait("Clone");
    let r3 = is_stdlib_trait("Clone");
    assert!(r1);
    assert!(r2);
    assert!(r3);
}
