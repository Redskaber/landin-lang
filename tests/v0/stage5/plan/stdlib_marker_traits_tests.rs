//! Stage 5.87: stdlib_marker_traits tests
//!
//! Tests `stdlib_marker_traits()` — returns all stdlib marker trait names
//! (Copy/Send/Sync/Sized/Unpin/Eq).
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{
    is_stdlib_marker_trait, stdlib_all_traits, stdlib_marker_traits, stdlib_traits_with_vtable,
};

// ============================================================
// Basic content tests
// ============================================================

/// stdlib_marker_traits returns a non-empty Vec.
#[test]
fn test_stdlib_marker_traits_non_empty() {
    let markers = stdlib_marker_traits();
    assert!(!markers.is_empty());
}

/// stdlib_marker_traits contains "Copy".
#[test]
fn test_stdlib_marker_traits_contains_copy() {
    let markers = stdlib_marker_traits();
    assert!(markers.contains(&"Copy"), "expected Copy in markers");
}

/// stdlib_marker_traits contains "Send".
#[test]
fn test_stdlib_marker_traits_contains_send() {
    let markers = stdlib_marker_traits();
    assert!(markers.contains(&"Send"), "expected Send in markers");
}

/// stdlib_marker_traits contains "Sync".
#[test]
fn test_stdlib_marker_traits_contains_sync() {
    let markers = stdlib_marker_traits();
    assert!(markers.contains(&"Sync"), "expected Sync in markers");
}

/// stdlib_marker_traits contains "Sized".
#[test]
fn test_stdlib_marker_traits_contains_sized() {
    let markers = stdlib_marker_traits();
    assert!(markers.contains(&"Sized"), "expected Sized in markers");
}

/// stdlib_marker_traits contains "Unpin".
#[test]
fn test_stdlib_marker_traits_contains_unpin() {
    let markers = stdlib_marker_traits();
    assert!(markers.contains(&"Unpin"), "expected Unpin in markers");
}

/// stdlib_marker_traits contains "Eq".
#[test]
fn test_stdlib_marker_traits_contains_eq() {
    let markers = stdlib_marker_traits();
    assert!(markers.contains(&"Eq"), "expected Eq in markers");
}

// ============================================================
// Exclusion tests
// ============================================================

/// stdlib_marker_traits does NOT contain "Clone" (method trait).
#[test]
fn test_stdlib_marker_traits_no_clone() {
    let markers = stdlib_marker_traits();
    assert!(!markers.contains(&"Clone"), "Clone should not be a marker");
}

/// stdlib_marker_traits does NOT contain "Drop" (method trait).
#[test]
fn test_stdlib_marker_traits_no_drop() {
    let markers = stdlib_marker_traits();
    assert!(!markers.contains(&"Drop"), "Drop should not be a marker");
}

/// stdlib_marker_traits does NOT contain "Foo" (user-defined).
#[test]
fn test_stdlib_marker_traits_no_foo() {
    let markers = stdlib_marker_traits();
    assert!(!markers.contains(&"Foo"), "Foo should not be in markers");
}

/// stdlib_marker_traits does NOT contain "Add" (arithmetic).
#[test]
fn test_stdlib_marker_traits_no_add() {
    let markers = stdlib_marker_traits();
    assert!(!markers.contains(&"Add"), "Add should not be a marker");
}

// ============================================================
// Count tests
// ============================================================

/// stdlib_marker_traits has exactly 6 entries (Copy/Send/Sync/Sized/Unpin/Eq).
#[test]
fn test_stdlib_marker_traits_count_6() {
    let markers = stdlib_marker_traits();
    assert_eq!(
        markers.len(),
        6,
        "expected 6 marker traits, got {}: {:?}",
        markers.len(),
        markers
    );
}

// ============================================================
// Consistency tests
// ============================================================

/// Every trait in stdlib_marker_traits returns true for is_stdlib_marker_trait.
#[test]
fn test_marker_traits_consistent_with_is_marker() {
    let markers = stdlib_marker_traits();
    for &name in &markers {
        assert!(
            is_stdlib_marker_trait(name),
            "trait {} in marker_traits but is_stdlib_marker_trait returned false",
            name
        );
    }
}

/// stdlib_marker_traits is a subset of stdlib_all_traits.
#[test]
fn test_marker_traits_subset_of_all_traits() {
    let markers = stdlib_marker_traits();
    let all = stdlib_all_traits();
    for &name in &markers {
        assert!(
            all.contains(&name),
            "marker trait {} not in all_traits",
            name
        );
    }
}

/// stdlib_marker_traits and stdlib_traits_with_vtable are disjoint.
#[test]
fn test_marker_traits_disjoint_from_with_vtable() {
    let markers = stdlib_marker_traits();
    let with_vtable = stdlib_traits_with_vtable();
    for &name in &markers {
        assert!(
            !with_vtable.contains(&name),
            "marker trait {} should not be in with_vtable (markers have no methods)",
            name
        );
    }
}

/// stdlib_marker_traits + stdlib_traits_with_vtable == stdlib_all_traits.
#[test]
fn test_markers_plus_vtable_equals_all() {
    let markers = stdlib_marker_traits();
    let with_vtable = stdlib_traits_with_vtable();
    let all = stdlib_all_traits();
    assert_eq!(
        markers.len() + with_vtable.len(),
        all.len(),
        "markers ({}) + with_vtable ({}) != all ({})",
        markers.len(),
        with_vtable.len(),
        all.len()
    );
}

// ============================================================
// Robustness tests
// ============================================================

/// No side effects — repeated calls return same result.
#[test]
fn test_stdlib_marker_traits_no_side_effects() {
    let m1 = stdlib_marker_traits();
    let m2 = stdlib_marker_traits();
    assert_eq!(m1, m2);
}

/// No duplicates in stdlib_marker_traits.
#[test]
fn test_stdlib_marker_traits_no_duplicates() {
    let markers = stdlib_marker_traits();
    let mut sorted = markers.clone();
    sorted.sort();
    let mut deduped = sorted.clone();
    deduped.dedup();
    assert_eq!(sorted, deduped, "found duplicates in stdlib_marker_traits");
}
