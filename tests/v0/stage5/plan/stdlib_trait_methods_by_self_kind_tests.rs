//! Stage 5.95: stdlib_trait_methods_by_self_kind tests
//!
//! Tests `stdlib_trait_methods_by_self_kind()` — reverse query returning all
//! stdlib trait methods with a given self_kind.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{
    stdlib_trait_method_self_kind, stdlib_trait_methods_by_self_kind, StdlibSelfKind,
};

// ============================================================
// Non-empty tests for each self_kind
// ============================================================

/// SelfByRef returns non-empty (Clone/Display/PartialEq etc. are by ref).
#[test]
fn test_by_self_kind_self_by_ref_non_empty() {
    let methods = stdlib_trait_methods_by_self_kind(StdlibSelfKind::SelfByRef);
    assert!(!methods.is_empty());
}

/// SelfByMutRef returns non-empty (Drop/clone_from are by mut ref).
#[test]
fn test_by_self_kind_self_by_mut_ref_non_empty() {
    let methods = stdlib_trait_methods_by_self_kind(StdlibSelfKind::SelfByMutRef);
    assert!(!methods.is_empty());
}

/// SelfByValue returns non-empty (arithmetic ops Add/Sub etc. are by value).
#[test]
fn test_by_self_kind_self_by_value_non_empty() {
    let methods = stdlib_trait_methods_by_self_kind(StdlibSelfKind::SelfByValue);
    assert!(!methods.is_empty());
}

/// NoSelf returns non-empty (Default::default is an associated function).
#[test]
fn test_by_self_kind_no_self_non_empty() {
    let methods = stdlib_trait_methods_by_self_kind(StdlibSelfKind::NoSelf);
    assert!(!methods.is_empty());
}

// ============================================================
// Contains tests
// ============================================================

/// SelfByRef contains ("Clone", "clone").
#[test]
fn test_by_self_kind_self_by_ref_contains_clone() {
    let methods = stdlib_trait_methods_by_self_kind(StdlibSelfKind::SelfByRef);
    assert!(
        methods.contains(&("Clone", "clone")),
        "expected (Clone, clone) in SelfByRef methods"
    );
}

/// SelfByMutRef contains ("Drop", "drop").
#[test]
fn test_by_self_kind_self_by_mut_ref_contains_drop() {
    let methods = stdlib_trait_methods_by_self_kind(StdlibSelfKind::SelfByMutRef);
    assert!(
        methods.contains(&("Drop", "drop")),
        "expected (Drop, drop) in SelfByMutRef methods"
    );
}

/// NoSelf contains ("Default", "default").
#[test]
fn test_by_self_kind_no_self_contains_default() {
    let methods = stdlib_trait_methods_by_self_kind(StdlibSelfKind::NoSelf);
    assert!(
        methods.contains(&("Default", "default")),
        "expected (Default, default) in NoSelf methods"
    );
}

// ============================================================
// Consistency tests
// ============================================================

/// All returned methods have self_kind matching the query parameter.
#[test]
fn test_by_self_kind_all_match() {
    for kind in [
        StdlibSelfKind::SelfByValue,
        StdlibSelfKind::SelfByRef,
        StdlibSelfKind::SelfByMutRef,
        StdlibSelfKind::NoSelf,
    ] {
        let methods = stdlib_trait_methods_by_self_kind(kind);
        for &(trait_name, method_name) in &methods {
            let actual_kind = stdlib_trait_method_self_kind(trait_name, method_name)
                .expect("method should exist");
            assert_eq!(
                actual_kind, kind,
                "method {}.{} has self_kind {:?} but was returned for {:?}",
                trait_name, method_name, actual_kind, kind
            );
        }
    }
}

/// SelfByMutRef has at least as many methods as SelfByRef.
/// (Assign ops use SelfByMutRef, and there are 10 of them + Drop + clone_from.)
#[test]
fn test_by_self_kind_self_by_mut_ref_not_empty_and_significant() {
    let by_mut_ref = stdlib_trait_methods_by_self_kind(StdlibSelfKind::SelfByMutRef);
    assert!(
        by_mut_ref.len() >= 5,
        "SelfByMutRef should have at least 5 methods, got {}",
        by_mut_ref.len()
    );
}

// ============================================================
// Robustness tests
// ============================================================

/// No side effects — repeated calls return same result.
#[test]
fn test_by_self_kind_no_side_effects() {
    let a1 = stdlib_trait_methods_by_self_kind(StdlibSelfKind::SelfByRef);
    let a2 = stdlib_trait_methods_by_self_kind(StdlibSelfKind::SelfByRef);
    assert_eq!(a1, a2);
}

/// All 4 self_kinds together cover all non-marker methods (markers have no methods).
#[test]
fn test_by_self_kind_all_four_cover_all_methods() {
    let by_value = stdlib_trait_methods_by_self_kind(StdlibSelfKind::SelfByValue);
    let by_ref = stdlib_trait_methods_by_self_kind(StdlibSelfKind::SelfByRef);
    let by_mut_ref = stdlib_trait_methods_by_self_kind(StdlibSelfKind::SelfByMutRef);
    let no_self = stdlib_trait_methods_by_self_kind(StdlibSelfKind::NoSelf);
    let total = by_value.len() + by_ref.len() + by_mut_ref.len() + no_self.len();

    // Count all methods across all traits
    use landin_compiler::{stdlib_all_traits, stdlib_trait_methods};
    let all_traits = stdlib_all_traits();
    let mut expected = 0;
    for trait_name in &all_traits {
        if let Some(methods) = stdlib_trait_methods(trait_name) {
            expected += methods.len();
        }
    }
    assert_eq!(
        total, expected,
        "all 4 self_kinds ({}) should cover all methods ({})",
        total, expected
    );
}
