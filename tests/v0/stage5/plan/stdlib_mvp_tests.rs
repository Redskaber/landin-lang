//! Stage 5.25: Stdlib MVP tests
//!
//! Tests `StdlibPrelude`, `register_stdlib()`, `default_prelude()`,
//! and the stdlib type/trait name constants.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::stdlib::{
    all_stdlib_trait_names, all_stdlib_type_names, default_prelude, register_stdlib,
    STDLIB_CONVERT_TRAITS, STDLIB_CORE_TYPES, STDLIB_ITER_TRAITS, STDLIB_OPS_TRAITS,
};
use lasso::Rodeo;

/// `STDLIB_CORE_TYPES` should contain all primitive types.
#[test]
fn test_stdlib_core_types() {
    assert!(STDLIB_CORE_TYPES.contains(&"i32"));
    assert!(STDLIB_CORE_TYPES.contains(&"bool"));
    assert!(STDLIB_CORE_TYPES.contains(&"str"));
    assert!(STDLIB_CORE_TYPES.contains(&"f64"));
    assert!(STDLIB_CORE_TYPES.contains(&"()"));
    assert_eq!(STDLIB_CORE_TYPES.len(), 17);
}

/// `STDLIB_OPS_TRAITS` should contain operator traits.
#[test]
fn test_stdlib_ops_traits() {
    assert!(STDLIB_OPS_TRAITS.contains(&"Add"));
    assert!(STDLIB_OPS_TRAITS.contains(&"Sub"));
    assert!(STDLIB_OPS_TRAITS.contains(&"PartialEq"));
    assert!(STDLIB_OPS_TRAITS.contains(&"Ord"));
}

/// `STDLIB_CONVERT_TRAITS` should contain conversion traits.
#[test]
fn test_stdlib_convert_traits() {
    assert!(STDLIB_CONVERT_TRAITS.contains(&"From"));
    assert!(STDLIB_CONVERT_TRAITS.contains(&"Into"));
    assert!(STDLIB_CONVERT_TRAITS.contains(&"AsRef"));
}

/// `STDLIB_ITER_TRAITS` should contain iterator traits.
#[test]
fn test_stdlib_iter_traits() {
    assert!(STDLIB_ITER_TRAITS.contains(&"Iterator"));
    assert!(STDLIB_ITER_TRAITS.contains(&"IntoIterator"));
}

/// `all_stdlib_trait_names()` should return deduplicated sorted list.
#[test]
fn test_all_stdlib_trait_names() {
    let names = all_stdlib_trait_names();
    assert!(!names.is_empty());
    // Check dedup: no consecutive duplicates after sort
    for i in 1..names.len() {
        assert_ne!(names[i], names[i - 1], "should be deduplicated");
    }
}

/// `all_stdlib_type_names()` should return core types.
#[test]
fn test_all_stdlib_type_names() {
    let names = all_stdlib_type_names();
    assert!(names.contains(&"i32"));
    assert!(names.contains(&"bool"));
}

/// `default_prelude()` should contain types + traits.
#[test]
fn test_default_prelude() {
    let prelude = default_prelude();
    assert!(prelude.contains("i32"));
    assert!(prelude.contains("Add"));
    assert!(prelude.contains("From"));
    assert!(prelude.contains("Iterator"));
    assert!(!prelude.is_empty());
}

/// `StdlibPrelude::len()` should return total count.
#[test]
fn test_prelude_len() {
    let prelude = default_prelude();
    let expected = all_stdlib_type_names().len() + all_stdlib_trait_names().len();
    assert_eq!(prelude.len(), expected);
}

/// `register_stdlib()` should intern all stdlib names.
#[test]
fn test_register_stdlib() {
    let mut interner = Rodeo::new();
    register_stdlib(&mut interner);
    // Core types should be interned
    assert!(interner.get("i32").is_some());
    assert!(interner.get("bool").is_some());
    assert!(interner.get("str").is_some());
    // Ops traits should be interned
    assert!(interner.get("Add").is_some());
    assert!(interner.get("PartialEq").is_some());
    // Convert traits should be interned
    assert!(interner.get("From").is_some());
    assert!(interner.get("Into").is_some());
    // Iter traits should be interned
    assert!(interner.get("Iterator").is_some());
}

/// `StdlibPrelude::contains()` should return false for unknown names.
#[test]
fn test_prelude_contains_false() {
    let prelude = default_prelude();
    assert!(!prelude.contains("MyCustomType"));
    assert!(!prelude.contains(""));
}
