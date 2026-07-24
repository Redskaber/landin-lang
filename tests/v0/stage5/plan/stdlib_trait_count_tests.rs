//! Stage 5.86: stdlib_trait_count + stdlib_all_traits tests
//!
//! Tests the new convenience query functions:
//! - `stdlib_trait_count()` — total number of stdlib traits
//! - `stdlib_all_traits()` — all stdlib trait names (marker + method)
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{
    is_stdlib_trait, stdlib_all_traits, stdlib_trait_count, stdlib_traits_with_vtable,
};

// ============================================================
// stdlib_trait_count tests
// ============================================================

/// stdlib_trait_count returns a positive number.
#[test]
fn test_stdlib_trait_count_positive() {
    let count = stdlib_trait_count();
    assert!(count > 0, "expected positive count, got {}", count);
}

/// stdlib_trait_count >= 30 (6 markers + ~24 method traits).
#[test]
fn test_stdlib_trait_count_at_least_30() {
    let count = stdlib_trait_count();
    assert!(
        count >= 30,
        "expected at least 30 stdlib traits, got {}",
        count
    );
}

/// stdlib_trait_count matches stdlib_all_traits().len().
#[test]
fn test_stdlib_trait_count_matches_all_traits_len() {
    let count = stdlib_trait_count();
    let all = stdlib_all_traits();
    assert_eq!(count, all.len());
}

// ============================================================
// stdlib_all_traits tests
// ============================================================

/// stdlib_all_traits returns a non-empty Vec.
#[test]
fn test_stdlib_all_traits_non_empty() {
    let all = stdlib_all_traits();
    assert!(!all.is_empty());
}

/// stdlib_all_traits contains "Copy" (marker trait).
#[test]
fn test_stdlib_all_traits_contains_copy() {
    let all = stdlib_all_traits();
    assert!(all.contains(&"Copy"), "expected Copy in all traits");
}

/// stdlib_all_traits contains "Clone" (method trait).
#[test]
fn test_stdlib_all_traits_contains_clone() {
    let all = stdlib_all_traits();
    assert!(all.contains(&"Clone"), "expected Clone in all traits");
}

/// stdlib_all_traits contains "Add" (arithmetic trait).
#[test]
fn test_stdlib_all_traits_contains_add() {
    let all = stdlib_all_traits();
    assert!(all.contains(&"Add"), "expected Add in all traits");
}

/// stdlib_all_traits contains "Drop" (core trait).
#[test]
fn test_stdlib_all_traits_contains_drop() {
    let all = stdlib_all_traits();
    assert!(all.contains(&"Drop"), "expected Drop in all traits");
}

/// stdlib_all_traits contains "ShrAssign" (assign trait).
#[test]
fn test_stdlib_all_traits_contains_shr_assign() {
    let all = stdlib_all_traits();
    assert!(
        all.contains(&"ShrAssign"),
        "expected ShrAssign in all traits"
    );
}

/// stdlib_all_traits does NOT contain "Foo" (user-defined).
#[test]
fn test_stdlib_all_traits_no_foo() {
    let all = stdlib_all_traits();
    assert!(!all.contains(&"Foo"), "Foo should not be in all traits");
}

/// stdlib_all_traits does NOT contain "" (empty string).
#[test]
fn test_stdlib_all_traits_no_empty() {
    let all = stdlib_all_traits();
    assert!(
        !all.contains(&""),
        "empty string should not be in all traits"
    );
}

/// stdlib_all_traits does NOT contain "clone" (method name, not trait).
#[test]
fn test_stdlib_all_traits_no_lowercase_clone() {
    let all = stdlib_all_traits();
    assert!(
        !all.contains(&"clone"),
        "lowercase 'clone' (method name) should not be in all traits"
    );
}

// ============================================================
// Consistency tests
// ============================================================

/// Every trait in stdlib_all_traits returns true for is_stdlib_trait.
#[test]
fn test_all_traits_consistent_with_is_stdlib_trait() {
    let all = stdlib_all_traits();
    for &trait_name in &all {
        assert!(
            is_stdlib_trait(trait_name),
            "trait {} in all_traits but is_stdlib_trait returned false",
            trait_name
        );
    }
}

/// stdlib_traits_with_vtable is a subset of stdlib_all_traits.
#[test]
fn test_with_vtable_subset_of_all_traits() {
    let all = stdlib_all_traits();
    let with_vtable = stdlib_traits_with_vtable();
    for &trait_name in &with_vtable {
        assert!(
            all.contains(&trait_name),
            "trait {} in with_vtable but not in all_traits",
            trait_name
        );
    }
}

/// stdlib_all_traits has more entries than stdlib_traits_with_vtable
/// (because all_traits includes markers, with_vtable excludes them).
#[test]
fn test_all_traits_more_than_with_vtable() {
    let all_count = stdlib_trait_count();
    let with_vtable_count = stdlib_traits_with_vtable().len();
    assert!(
        all_count > with_vtable_count,
        "all_traits ({}) should have more than with_vtable ({}) because markers are excluded from vtable",
        all_count,
        with_vtable_count
    );
}

/// No side effects — repeated calls return same result.
#[test]
fn test_stdlib_all_traits_no_side_effects() {
    let a1 = stdlib_all_traits();
    let a2 = stdlib_all_traits();
    assert_eq!(a1, a2);
}

/// No duplicates in stdlib_all_traits.
#[test]
fn test_stdlib_all_traits_no_duplicates() {
    let all = stdlib_all_traits();
    let mut sorted = all.clone();
    sorted.sort();
    let mut deduped = sorted.clone();
    deduped.dedup();
    assert_eq!(sorted, deduped, "found duplicates in stdlib_all_traits");
}
