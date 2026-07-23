//! Stage 5.26: Driver stdlib integration tests
//!
//! Tests that `register_stdlib()` is called by the driver and
//! `CompileResult.stdlib_prelude` is populated.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;

/// `CompileResult.stdlib_prelude` should be populated after compile.
#[test]
fn test_stdlib_prelude_populated() {
    let result = compile("fn main() {}");
    assert!(
        !result.stdlib_prelude.is_empty(),
        "stdlib_prelude should be populated"
    );
}

/// Stdlib types should be interned after compile.
#[test]
fn test_stdlib_types_interned() {
    let result = compile("fn main() {}");
    assert!(
        result.interner.get("i32").is_some(),
        "i32 should be interned"
    );
    assert!(
        result.interner.get("bool").is_some(),
        "bool should be interned"
    );
    assert!(
        result.interner.get("str").is_some(),
        "str should be interned"
    );
    assert!(
        result.interner.get("f64").is_some(),
        "f64 should be interned"
    );
}

/// Stdlib ops traits should be interned after compile.
#[test]
fn test_stdlib_ops_traits_interned() {
    let result = compile("fn main() {}");
    assert!(
        result.interner.get("Add").is_some(),
        "Add should be interned"
    );
    assert!(
        result.interner.get("PartialEq").is_some(),
        "PartialEq should be interned"
    );
    assert!(
        result.interner.get("Ord").is_some(),
        "Ord should be interned"
    );
}

/// Stdlib convert traits should be interned after compile.
#[test]
fn test_stdlib_convert_traits_interned() {
    let result = compile("fn main() {}");
    assert!(
        result.interner.get("From").is_some(),
        "From should be interned"
    );
    assert!(
        result.interner.get("Into").is_some(),
        "Into should be interned"
    );
    assert!(
        result.interner.get("AsRef").is_some(),
        "AsRef should be interned"
    );
}

/// Stdlib iter traits should be interned after compile.
#[test]
fn test_stdlib_iter_traits_interned() {
    let result = compile("fn main() {}");
    assert!(
        result.interner.get("Iterator").is_some(),
        "Iterator should be interned"
    );
}

/// `stdlib_prelude.contains()` should work for stdlib types.
#[test]
fn test_prelude_contains_types() {
    let result = compile("fn main() {}");
    assert!(result.stdlib_prelude.contains("i32"));
    assert!(result.stdlib_prelude.contains("bool"));
    assert!(!result.stdlib_prelude.contains("MyCustomType"));
}

/// `stdlib_prelude.contains()` should work for stdlib traits.
#[test]
fn test_prelude_contains_traits() {
    let result = compile("fn main() {}");
    assert!(result.stdlib_prelude.contains("Add"));
    assert!(result.stdlib_prelude.contains("From"));
    assert!(result.stdlib_prelude.contains("Iterator"));
    assert!(!result.stdlib_prelude.contains("MyCustomTrait"));
}

/// Lex error path should still have stdlib_prelude (via empty()).
#[test]
fn test_stdlib_prelude_on_lex_error() {
    let result = compile("!!!invalid tokens!!!");
    // Even on lex error, stdlib_prelude should be populated (via empty()).
    assert!(
        !result.stdlib_prelude.is_empty(),
        "stdlib_prelude should be populated even on lex error"
    );
}
