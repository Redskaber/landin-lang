//! Stage 5.99: stdlib_trait_methods_by_param_count tests
//!
//! Tests `stdlib_trait_methods_by_param_count()` — reverse query returning all
//! stdlib trait methods with a given parameter count.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::{stdlib_trait_method_param_count, stdlib_trait_methods_by_param_count};

/// param_count=0 returns non-empty (Drop::drop, Clone::clone, etc.).
#[test]
fn test_by_param_count_zero_non_empty() {
    let methods = stdlib_trait_methods_by_param_count(0);
    assert!(!methods.is_empty());
}

/// param_count=1 returns non-empty (Display::fmt, PartialEq::eq, etc.).
#[test]
fn test_by_param_count_one_non_empty() {
    let methods = stdlib_trait_methods_by_param_count(1);
    assert!(!methods.is_empty());
}

/// param_count=0 contains ("Drop", "drop").
#[test]
fn test_by_param_count_zero_contains_drop() {
    let methods = stdlib_trait_methods_by_param_count(0);
    assert!(
        methods.contains(&("Drop", "drop")),
        "expected (Drop, drop) in 0-param methods"
    );
}

/// param_count=1 contains ("Display", "fmt").
#[test]
fn test_by_param_count_one_contains_fmt() {
    let methods = stdlib_trait_methods_by_param_count(1);
    assert!(
        methods.contains(&("Display", "fmt")),
        "expected (Display, fmt) in 1-param methods"
    );
}

/// param_count=99 returns empty (no method has 99 params).
#[test]
fn test_by_param_count_99_empty() {
    let methods = stdlib_trait_methods_by_param_count(99);
    assert!(
        methods.is_empty(),
        "expected no methods with 99 params, got {:?}",
        methods
    );
}

/// All returned methods have param_count matching the query parameter.
#[test]
fn test_by_param_count_all_match() {
    for count in 0..=3 {
        let methods = stdlib_trait_methods_by_param_count(count);
        for &(trait_name, method_name) in &methods {
            let actual = stdlib_trait_method_param_count(trait_name, method_name)
                .expect("method should exist");
            assert_eq!(
                actual, count,
                "method {}.{} has param_count={} but was returned for {}",
                trait_name, method_name, actual, count
            );
        }
    }
}

/// No side effects — repeated calls return same result.
#[test]
fn test_by_param_count_no_side_effects() {
    let a1 = stdlib_trait_methods_by_param_count(0);
    let a2 = stdlib_trait_methods_by_param_count(0);
    assert_eq!(a1, a2);
}
