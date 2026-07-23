//! Stage 5.33: Stdlib facade driver integration tests
//!
//! Tests that `CompileResult.stdlib_facade` is populated and queryable
//! after compilation.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;
use landin_compiler::stdlib::StdlibLayer;

/// `CompileResult.stdlib_facade` should be populated after compile.
#[test]
fn test_facade_populated() {
    let result = compile("fn main() {}");
    assert_eq!(result.stdlib_facade.layer_count(), 3);
    assert!(result.stdlib_facade.type_count() > 0);
}

/// `stdlib_facade.type_count()` should be 57 (17+13+27).
#[test]
fn test_facade_type_count_in_result() {
    let result = compile("fn main() {}");
    assert_eq!(result.stdlib_facade.type_count(), 57);
}

/// `stdlib_facade.is_stdlib_name()` should work via CompileResult.
#[test]
fn test_facade_is_stdlib_name_in_result() {
    let result = compile("fn main() {}");
    assert!(result.stdlib_facade.is_stdlib_name("i32"));
    assert!(result.stdlib_facade.is_stdlib_name("Box"));
    assert!(result.stdlib_facade.is_stdlib_name("File"));
    assert!(!result.stdlib_facade.is_stdlib_name("MyType"));
}

/// `stdlib_facade.summary()` should be accessible via CompileResult.
#[test]
fn test_facade_summary_in_result() {
    let result = compile("fn main() {}");
    let s = result.stdlib_facade.summary();
    assert!(s.contains("StdlibFacade:"));
    assert!(s.contains("layers: 3"));
}

/// `stdlib_facade.type_count_for_layer()` should work via CompileResult.
#[test]
fn test_facade_type_count_for_layer_in_result() {
    let result = compile("fn main() {}");
    assert_eq!(
        result.stdlib_facade.type_count_for_layer(StdlibLayer::Core),
        17
    );
    assert_eq!(
        result
            .stdlib_facade
            .type_count_for_layer(StdlibLayer::Alloc),
        13
    );
    assert_eq!(
        result.stdlib_facade.type_count_for_layer(StdlibLayer::Std),
        27
    );
}

/// Facade should be available even on lex error path (via empty()).
#[test]
fn test_facade_on_lex_error() {
    let result = compile("!!!invalid tokens!!!");
    assert_eq!(result.stdlib_facade.layer_count(), 3);
    assert!(result.stdlib_facade.type_count() > 0);
}

/// `stdlib_facade.trait_count()` should be > 0.
#[test]
fn test_facade_trait_count_in_result() {
    let result = compile("fn main() {}");
    assert!(result.stdlib_facade.trait_count() > 0);
}
