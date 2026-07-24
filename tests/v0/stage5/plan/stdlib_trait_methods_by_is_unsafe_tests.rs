//! Stage 5.98: stdlib_trait_methods_by_is_unsafe tests
//!
//! Tests `stdlib_trait_methods_by_is_unsafe()` — reverse query returning all
//! stdlib trait methods matching a given is_unsafe flag.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{
    stdlib_all_traits, stdlib_trait_method_is_unsafe, stdlib_trait_methods,
    stdlib_trait_methods_by_is_unsafe,
};

/// is_unsafe=false returns non-empty (all current methods are safe).
#[test]
fn test_by_is_unsafe_false_non_empty() {
    let methods = stdlib_trait_methods_by_is_unsafe(false);
    assert!(!methods.is_empty());
}

/// is_unsafe=true returns empty (no current stdlib methods are unsafe).
#[test]
fn test_by_is_unsafe_true_empty() {
    let methods = stdlib_trait_methods_by_is_unsafe(true);
    assert!(
        methods.is_empty(),
        "expected no unsafe methods, got {:?}",
        methods
    );
}

/// is_unsafe=false contains ("Clone", "clone").
#[test]
fn test_by_is_unsafe_false_contains_clone() {
    let methods = stdlib_trait_methods_by_is_unsafe(false);
    assert!(
        methods.contains(&("Clone", "clone")),
        "expected (Clone, clone) in safe methods"
    );
}

/// is_unsafe=false contains ("Drop", "drop").
#[test]
fn test_by_is_unsafe_false_contains_drop() {
    let methods = stdlib_trait_methods_by_is_unsafe(false);
    assert!(
        methods.contains(&("Drop", "drop")),
        "expected (Drop, drop) in safe methods"
    );
}

/// All returned methods have is_unsafe matching the query parameter.
#[test]
fn test_by_is_unsafe_all_match() {
    for flag in [true, false] {
        let methods = stdlib_trait_methods_by_is_unsafe(flag);
        for &(trait_name, method_name) in &methods {
            let actual = stdlib_trait_method_is_unsafe(trait_name, method_name)
                .expect("method should exist");
            assert_eq!(
                actual, flag,
                "method {}.{} has is_unsafe={} but was returned for {}",
                trait_name, method_name, actual, flag
            );
        }
    }
}

/// false + true together cover all non-marker methods.
#[test]
fn test_by_is_unsafe_both_cover_all_methods() {
    let safe = stdlib_trait_methods_by_is_unsafe(false);
    let unsafe_m = stdlib_trait_methods_by_is_unsafe(true);
    let total = safe.len() + unsafe_m.len();

    let all_traits = stdlib_all_traits();
    let mut expected = 0;
    for trait_name in &all_traits {
        if let Some(methods) = stdlib_trait_methods(trait_name) {
            expected += methods.len();
        }
    }
    assert_eq!(total, expected);
}

/// No side effects — repeated calls return same result.
#[test]
fn test_by_is_unsafe_no_side_effects() {
    let a1 = stdlib_trait_methods_by_is_unsafe(false);
    let a2 = stdlib_trait_methods_by_is_unsafe(false);
    assert_eq!(a1, a2);
}
