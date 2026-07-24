//! Stage 5.89: stdlib_core_traits tests
//!
//! Tests `stdlib_core_traits()` — returns all stdlib core trait names
//! (Clone/Drop/Default/Display/Debug/PartialEq/PartialOrd/Ord/Hash/
//! Deref/DerefMut/IntoIterator/Iterator).
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{
    stdlib_all_traits, stdlib_arithmetic_traits, stdlib_core_traits, stdlib_marker_traits,
};

// ============================================================
// Basic content tests
// ============================================================

/// stdlib_core_traits returns a non-empty Vec.
#[test]
fn test_stdlib_core_traits_non_empty() {
    let core = stdlib_core_traits();
    assert!(!core.is_empty());
}

/// stdlib_core_traits contains "Clone".
#[test]
fn test_stdlib_core_traits_contains_clone() {
    let core = stdlib_core_traits();
    assert!(core.contains(&"Clone"), "expected Clone in core traits");
}

/// stdlib_core_traits contains "Drop".
#[test]
fn test_stdlib_core_traits_contains_drop() {
    let core = stdlib_core_traits();
    assert!(core.contains(&"Drop"), "expected Drop in core traits");
}

/// stdlib_core_traits contains "Default".
#[test]
fn test_stdlib_core_traits_contains_default() {
    let core = stdlib_core_traits();
    assert!(core.contains(&"Default"), "expected Default in core traits");
}

/// stdlib_core_traits contains "Display".
#[test]
fn test_stdlib_core_traits_contains_display() {
    let core = stdlib_core_traits();
    assert!(core.contains(&"Display"), "expected Display in core traits");
}

/// stdlib_core_traits contains "Debug".
#[test]
fn test_stdlib_core_traits_contains_debug() {
    let core = stdlib_core_traits();
    assert!(core.contains(&"Debug"), "expected Debug in core traits");
}

/// stdlib_core_traits contains "PartialEq".
#[test]
fn test_stdlib_core_traits_contains_partial_eq() {
    let core = stdlib_core_traits();
    assert!(
        core.contains(&"PartialEq"),
        "expected PartialEq in core traits"
    );
}

/// stdlib_core_traits contains "Ord".
#[test]
fn test_stdlib_core_traits_contains_ord() {
    let core = stdlib_core_traits();
    assert!(core.contains(&"Ord"), "expected Ord in core traits");
}

/// stdlib_core_traits contains "Hash".
#[test]
fn test_stdlib_core_traits_contains_hash() {
    let core = stdlib_core_traits();
    assert!(core.contains(&"Hash"), "expected Hash in core traits");
}

/// stdlib_core_traits contains "Deref".
#[test]
fn test_stdlib_core_traits_contains_deref() {
    let core = stdlib_core_traits();
    assert!(core.contains(&"Deref"), "expected Deref in core traits");
}

/// stdlib_core_traits contains "Iterator".
#[test]
fn test_stdlib_core_traits_contains_iterator() {
    let core = stdlib_core_traits();
    assert!(
        core.contains(&"Iterator"),
        "expected Iterator in core traits"
    );
}

/// stdlib_core_traits contains "IntoIterator".
#[test]
fn test_stdlib_core_traits_contains_into_iterator() {
    let core = stdlib_core_traits();
    assert!(
        core.contains(&"IntoIterator"),
        "expected IntoIterator in core traits"
    );
}

// ============================================================
// Exclusion tests
// ============================================================

/// stdlib_core_traits does NOT contain "Copy" (marker).
#[test]
fn test_stdlib_core_traits_no_copy() {
    let core = stdlib_core_traits();
    assert!(!core.contains(&"Copy"), "Copy should not be in core traits");
}

/// stdlib_core_traits does NOT contain "Add" (arithmetic).
#[test]
fn test_stdlib_core_traits_no_add() {
    let core = stdlib_core_traits();
    assert!(!core.contains(&"Add"), "Add should not be in core traits");
}

/// stdlib_core_traits does NOT contain "Foo" (user-defined).
#[test]
fn test_stdlib_core_traits_no_foo() {
    let core = stdlib_core_traits();
    assert!(!core.contains(&"Foo"), "Foo should not be in core traits");
}

/// stdlib_core_traits does NOT contain "Read" (I/O trait).
#[test]
fn test_stdlib_core_traits_no_read() {
    let core = stdlib_core_traits();
    assert!(!core.contains(&"Read"), "Read should not be in core traits");
}

// ============================================================
// Count tests
// ============================================================

/// stdlib_core_traits has exactly 13 entries.
#[test]
fn test_stdlib_core_traits_count_13() {
    let core = stdlib_core_traits();
    assert_eq!(
        core.len(),
        13,
        "expected 13 core traits, got {}: {:?}",
        core.len(),
        core
    );
}

// ============================================================
// Consistency tests
// ============================================================

/// stdlib_core_traits is a subset of stdlib_all_traits.
#[test]
fn test_core_traits_subset_of_all_traits() {
    let core = stdlib_core_traits();
    let all = stdlib_all_traits();
    for &name in &core {
        assert!(all.contains(&name), "core trait {} not in all_traits", name);
    }
}

/// stdlib_core_traits and stdlib_marker_traits are disjoint.
#[test]
fn test_core_traits_disjoint_from_markers() {
    let core = stdlib_core_traits();
    let markers = stdlib_marker_traits();
    for &name in &core {
        assert!(
            !markers.contains(&name),
            "core trait {} should not be a marker",
            name
        );
    }
}

/// stdlib_core_traits and stdlib_arithmetic_traits are disjoint.
#[test]
fn test_core_traits_disjoint_from_arithmetic() {
    let core = stdlib_core_traits();
    let arith = stdlib_arithmetic_traits();
    for &name in &core {
        assert!(
            !arith.contains(&name),
            "core trait {} should not be arithmetic",
            name
        );
    }
}

// ============================================================
// Robustness tests
// ============================================================

/// No side effects — repeated calls return same result.
#[test]
fn test_stdlib_core_traits_no_side_effects() {
    let c1 = stdlib_core_traits();
    let c2 = stdlib_core_traits();
    assert_eq!(c1, c2);
}

/// No duplicates in stdlib_core_traits.
#[test]
fn test_stdlib_core_traits_no_duplicates() {
    let core = stdlib_core_traits();
    let mut sorted = core.clone();
    sorted.sort();
    let mut deduped = sorted.clone();
    deduped.dedup();
    assert_eq!(sorted, deduped, "found duplicates in stdlib_core_traits");
}
