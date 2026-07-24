//! Stage 5.90: stdlib_io_traits + stdlib_unary_traits tests
//!
//! Tests the two new semantic group query functions:
//! - `stdlib_io_traits()` — returns I/O traits (Read, Write)
//! - `stdlib_unary_traits()` — returns unary operator traits (Neg, Not)
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{
    stdlib_all_traits, stdlib_arithmetic_traits, stdlib_io_traits, stdlib_marker_traits,
    stdlib_unary_traits,
};

// ============================================================
// stdlib_io_traits tests
// ============================================================

/// stdlib_io_traits returns a non-empty Vec.
#[test]
fn test_stdlib_io_traits_non_empty() {
    let io = stdlib_io_traits();
    assert!(!io.is_empty());
}

/// stdlib_io_traits contains "Read".
#[test]
fn test_stdlib_io_traits_contains_read() {
    let io = stdlib_io_traits();
    assert!(io.contains(&"Read"), "expected Read in io traits");
}

/// stdlib_io_traits contains "Write".
#[test]
fn test_stdlib_io_traits_contains_write() {
    let io = stdlib_io_traits();
    assert!(io.contains(&"Write"), "expected Write in io traits");
}

/// stdlib_io_traits has exactly 2 entries.
#[test]
fn test_stdlib_io_traits_count_2() {
    let io = stdlib_io_traits();
    assert_eq!(
        io.len(),
        2,
        "expected 2 io traits, got {}: {:?}",
        io.len(),
        io
    );
}

/// stdlib_io_traits does NOT contain "Copy" (marker).
#[test]
fn test_stdlib_io_traits_no_copy() {
    let io = stdlib_io_traits();
    assert!(!io.contains(&"Copy"), "Copy should not be in io traits");
}

/// stdlib_io_traits does NOT contain "Foo" (user-defined).
#[test]
fn test_stdlib_io_traits_no_foo() {
    let io = stdlib_io_traits();
    assert!(!io.contains(&"Foo"), "Foo should not be in io traits");
}

/// stdlib_io_traits is a subset of stdlib_all_traits.
#[test]
fn test_io_traits_subset_of_all_traits() {
    let io = stdlib_io_traits();
    let all = stdlib_all_traits();
    for &name in &io {
        assert!(all.contains(&name), "io trait {} not in all_traits", name);
    }
}

/// stdlib_io_traits and stdlib_marker_traits are disjoint.
#[test]
fn test_io_traits_disjoint_from_markers() {
    let io = stdlib_io_traits();
    let markers = stdlib_marker_traits();
    for &name in &io {
        assert!(
            !markers.contains(&name),
            "io trait {} should not be a marker",
            name
        );
    }
}

// ============================================================
// stdlib_unary_traits tests
// ============================================================

/// stdlib_unary_traits returns a non-empty Vec.
#[test]
fn test_stdlib_unary_traits_non_empty() {
    let unary = stdlib_unary_traits();
    assert!(!unary.is_empty());
}

/// stdlib_unary_traits contains "Neg".
#[test]
fn test_stdlib_unary_traits_contains_neg() {
    let unary = stdlib_unary_traits();
    assert!(unary.contains(&"Neg"), "expected Neg in unary traits");
}

/// stdlib_unary_traits contains "Not".
#[test]
fn test_stdlib_unary_traits_contains_not() {
    let unary = stdlib_unary_traits();
    assert!(unary.contains(&"Not"), "expected Not in unary traits");
}

/// stdlib_unary_traits has exactly 2 entries.
#[test]
fn test_stdlib_unary_traits_count_2() {
    let unary = stdlib_unary_traits();
    assert_eq!(
        unary.len(),
        2,
        "expected 2 unary traits, got {}: {:?}",
        unary.len(),
        unary
    );
}

/// stdlib_unary_traits does NOT contain "Copy" (marker).
#[test]
fn test_stdlib_unary_traits_no_copy() {
    let unary = stdlib_unary_traits();
    assert!(
        !unary.contains(&"Copy"),
        "Copy should not be in unary traits"
    );
}

/// stdlib_unary_traits does NOT contain "Add" (arithmetic binary).
#[test]
fn test_stdlib_unary_traits_no_add() {
    let unary = stdlib_unary_traits();
    assert!(!unary.contains(&"Add"), "Add should not be in unary traits");
}

/// stdlib_unary_traits is a subset of stdlib_all_traits.
#[test]
fn test_unary_traits_subset_of_all_traits() {
    let unary = stdlib_unary_traits();
    let all = stdlib_all_traits();
    for &name in &unary {
        assert!(
            all.contains(&name),
            "unary trait {} not in all_traits",
            name
        );
    }
}

/// stdlib_unary_traits and stdlib_arithmetic_traits are disjoint.
#[test]
fn test_unary_traits_disjoint_from_arithmetic() {
    let unary = stdlib_unary_traits();
    let arith = stdlib_arithmetic_traits();
    for &name in &unary {
        assert!(
            !arith.contains(&name),
            "unary trait {} should not be arithmetic",
            name
        );
    }
}

// ============================================================
// Robustness tests (both functions)
// ============================================================

/// No side effects — repeated calls return same result (io_traits).
#[test]
fn test_stdlib_io_traits_no_side_effects() {
    let a1 = stdlib_io_traits();
    let a2 = stdlib_io_traits();
    assert_eq!(a1, a2);
}

/// No side effects — repeated calls return same result (unary_traits).
#[test]
fn test_stdlib_unary_traits_no_side_effects() {
    let a1 = stdlib_unary_traits();
    let a2 = stdlib_unary_traits();
    assert_eq!(a1, a2);
}

/// No duplicates in stdlib_io_traits.
#[test]
fn test_stdlib_io_traits_no_duplicates() {
    let io = stdlib_io_traits();
    let mut sorted = io.clone();
    sorted.sort();
    let mut deduped = sorted.clone();
    deduped.dedup();
    assert_eq!(sorted, deduped, "found duplicates in stdlib_io_traits");
}

/// No duplicates in stdlib_unary_traits.
#[test]
fn test_stdlib_unary_traits_no_duplicates() {
    let unary = stdlib_unary_traits();
    let mut sorted = unary.clone();
    sorted.sort();
    let mut deduped = sorted.clone();
    deduped.dedup();
    assert_eq!(sorted, deduped, "found duplicates in stdlib_unary_traits");
}

/// stdlib_io_traits and stdlib_unary_traits are disjoint.
#[test]
fn test_io_traits_disjoint_from_unary() {
    let io = stdlib_io_traits();
    let unary = stdlib_unary_traits();
    for &name in &io {
        assert!(
            !unary.contains(&name),
            "io trait {} should not be unary",
            name
        );
    }
}
