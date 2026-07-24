//! Stage 5.88: stdlib_arithmetic_traits tests
//!
//! Tests `stdlib_arithmetic_traits()` — returns all stdlib arithmetic
//! operator trait names (binary ops + assign variants).
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{stdlib_all_traits, stdlib_arithmetic_traits, stdlib_marker_traits};

// ============================================================
// Basic content tests — binary arithmetic ops
// ============================================================

/// stdlib_arithmetic_traits returns a non-empty Vec.
#[test]
fn test_stdlib_arithmetic_traits_non_empty() {
    let arith = stdlib_arithmetic_traits();
    assert!(!arith.is_empty());
}

/// stdlib_arithmetic_traits contains "Add".
#[test]
fn test_stdlib_arithmetic_traits_contains_add() {
    let arith = stdlib_arithmetic_traits();
    assert!(arith.contains(&"Add"), "expected Add in arithmetic traits");
}

/// stdlib_arithmetic_traits contains "Sub".
#[test]
fn test_stdlib_arithmetic_traits_contains_sub() {
    let arith = stdlib_arithmetic_traits();
    assert!(arith.contains(&"Sub"), "expected Sub in arithmetic traits");
}

/// stdlib_arithmetic_traits contains "Mul".
#[test]
fn test_stdlib_arithmetic_traits_contains_mul() {
    let arith = stdlib_arithmetic_traits();
    assert!(arith.contains(&"Mul"), "expected Mul in arithmetic traits");
}

/// stdlib_arithmetic_traits contains "Div".
#[test]
fn test_stdlib_arithmetic_traits_contains_div() {
    let arith = stdlib_arithmetic_traits();
    assert!(arith.contains(&"Div"), "expected Div in arithmetic traits");
}

/// stdlib_arithmetic_traits contains "Rem".
#[test]
fn test_stdlib_arithmetic_traits_contains_rem() {
    let arith = stdlib_arithmetic_traits();
    assert!(arith.contains(&"Rem"), "expected Rem in arithmetic traits");
}

/// stdlib_arithmetic_traits contains "BitAnd".
#[test]
fn test_stdlib_arithmetic_traits_contains_bitand() {
    let arith = stdlib_arithmetic_traits();
    assert!(
        arith.contains(&"BitAnd"),
        "expected BitAnd in arithmetic traits"
    );
}

/// stdlib_arithmetic_traits contains "Shl".
#[test]
fn test_stdlib_arithmetic_traits_contains_shl() {
    let arith = stdlib_arithmetic_traits();
    assert!(arith.contains(&"Shl"), "expected Shl in arithmetic traits");
}

/// stdlib_arithmetic_traits contains "Shr".
#[test]
fn test_stdlib_arithmetic_traits_contains_shr() {
    let arith = stdlib_arithmetic_traits();
    assert!(arith.contains(&"Shr"), "expected Shr in arithmetic traits");
}

// ============================================================
// Assign variant tests
// ============================================================

/// stdlib_arithmetic_traits contains "AddAssign".
#[test]
fn test_stdlib_arithmetic_traits_contains_add_assign() {
    let arith = stdlib_arithmetic_traits();
    assert!(
        arith.contains(&"AddAssign"),
        "expected AddAssign in arithmetic traits"
    );
}

/// stdlib_arithmetic_traits contains "ShrAssign" (last assign variant).
#[test]
fn test_stdlib_arithmetic_traits_contains_shr_assign() {
    let arith = stdlib_arithmetic_traits();
    assert!(
        arith.contains(&"ShrAssign"),
        "expected ShrAssign in arithmetic traits"
    );
}

// ============================================================
// Exclusion tests
// ============================================================

/// stdlib_arithmetic_traits does NOT contain "Copy" (marker).
#[test]
fn test_stdlib_arithmetic_traits_no_copy() {
    let arith = stdlib_arithmetic_traits();
    assert!(
        !arith.contains(&"Copy"),
        "Copy should not be in arithmetic traits"
    );
}

/// stdlib_arithmetic_traits does NOT contain "Clone" (core trait).
#[test]
fn test_stdlib_arithmetic_traits_no_clone() {
    let arith = stdlib_arithmetic_traits();
    assert!(
        !arith.contains(&"Clone"),
        "Clone should not be in arithmetic traits"
    );
}

/// stdlib_arithmetic_traits does NOT contain "Foo" (user-defined).
#[test]
fn test_stdlib_arithmetic_traits_no_foo() {
    let arith = stdlib_arithmetic_traits();
    assert!(
        !arith.contains(&"Foo"),
        "Foo should not be in arithmetic traits"
    );
}

/// stdlib_arithmetic_traits does NOT contain "Drop" (core trait).
#[test]
fn test_stdlib_arithmetic_traits_no_drop() {
    let arith = stdlib_arithmetic_traits();
    assert!(
        !arith.contains(&"Drop"),
        "Drop should not be in arithmetic traits"
    );
}

// ============================================================
// Count tests
// ============================================================

/// stdlib_arithmetic_traits has exactly 20 entries (10 binary + 10 assign).
#[test]
fn test_stdlib_arithmetic_traits_count_20() {
    let arith = stdlib_arithmetic_traits();
    assert_eq!(
        arith.len(),
        20,
        "expected 20 arithmetic traits, got {}: {:?}",
        arith.len(),
        arith
    );
}

// ============================================================
// Consistency tests
// ============================================================

/// stdlib_arithmetic_traits is a subset of stdlib_all_traits.
#[test]
fn test_arithmetic_traits_subset_of_all_traits() {
    let arith = stdlib_arithmetic_traits();
    let all = stdlib_all_traits();
    for &name in &arith {
        assert!(
            all.contains(&name),
            "arithmetic trait {} not in all_traits",
            name
        );
    }
}

/// stdlib_arithmetic_traits and stdlib_marker_traits are disjoint.
#[test]
fn test_arithmetic_traits_disjoint_from_markers() {
    let arith = stdlib_arithmetic_traits();
    let markers = stdlib_marker_traits();
    for &name in &arith {
        assert!(
            !markers.contains(&name),
            "arithmetic trait {} should not be a marker",
            name
        );
    }
}

// ============================================================
// Robustness tests
// ============================================================

/// No side effects — repeated calls return same result.
#[test]
fn test_stdlib_arithmetic_traits_no_side_effects() {
    let a1 = stdlib_arithmetic_traits();
    let a2 = stdlib_arithmetic_traits();
    assert_eq!(a1, a2);
}

/// No duplicates in stdlib_arithmetic_traits.
#[test]
fn test_stdlib_arithmetic_traits_no_duplicates() {
    let arith = stdlib_arithmetic_traits();
    let mut sorted = arith.clone();
    sorted.sort();
    let mut deduped = sorted.clone();
    deduped.dedup();
    assert_eq!(
        sorted, deduped,
        "found duplicates in stdlib_arithmetic_traits"
    );
}
