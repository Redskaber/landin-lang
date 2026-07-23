//! Stage 5.31: Stdlib facade tests
//!
//! Tests `StdlibFacade` struct: `type_count()`, `trait_count()`,
//! `type_count_for_layer()`, `layer_count()`, `is_stdlib_name()`, `summary()`.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;
use landin_compiler::stdlib::StdlibFacade;

/// `StdlibFacade::default()` should have correct type count.
#[test]
fn test_facade_type_count() {
    let facade = StdlibFacade::default();
    let count = facade.type_count();
    // core (17) + alloc (13) + std (27) = 57
    assert_eq!(count, 57, "should have 57 total types");
}

/// `StdlibFacade::default()` should have correct trait count (> 0).
#[test]
fn test_facade_trait_count() {
    let facade = StdlibFacade::default();
    assert!(facade.trait_count() > 0, "should have traits");
}

/// `type_count_for_layer` should return correct counts.
#[test]
fn test_facade_type_count_for_layer() {
    let facade = StdlibFacade::default();
    assert_eq!(
        facade.type_count_for_layer(landin_compiler::StdlibLayer::Core),
        17
    );
    assert_eq!(
        facade.type_count_for_layer(landin_compiler::StdlibLayer::Alloc),
        13
    );
    assert_eq!(
        facade.type_count_for_layer(landin_compiler::StdlibLayer::Std),
        27
    );
    assert_eq!(
        facade.type_count_for_layer(landin_compiler::StdlibLayer::None),
        0
    );
}

/// `layer_count` should always be 3.
#[test]
fn test_facade_layer_count() {
    let facade = StdlibFacade::default();
    assert_eq!(facade.layer_count(), 3);
}

/// `is_stdlib_name` should return true for stdlib types + traits.
#[test]
fn test_facade_is_stdlib_name() {
    let facade = StdlibFacade::default();
    // Core types
    assert!(facade.is_stdlib_name("i32"));
    assert!(facade.is_stdlib_name("bool"));
    // Alloc types
    assert!(facade.is_stdlib_name("Box"));
    assert!(facade.is_stdlib_name("Vec"));
    // Std types
    assert!(facade.is_stdlib_name("File"));
    assert!(facade.is_stdlib_name("Result"));
    // Traits
    assert!(facade.is_stdlib_name("Add"));
    assert!(facade.is_stdlib_name("Display"));
    assert!(facade.is_stdlib_name("Read"));
    // Non-stdlib
    assert!(!facade.is_stdlib_name("MyType"));
    assert!(!facade.is_stdlib_name(""));
}

/// `summary()` should contain key information.
#[test]
fn test_facade_summary() {
    let facade = StdlibFacade::default();
    let s = facade.summary();
    assert!(s.contains("StdlibFacade:"));
    assert!(s.contains("layers: 3"));
    assert!(s.contains("total types: 57"));
}

/// `from_prelude` should create facade from existing prelude.
#[test]
fn test_facade_from_prelude() {
    let result = compile("fn main() {}");
    let facade = StdlibFacade::from_prelude(result.stdlib_prelude.clone());
    assert_eq!(facade.type_count(), 57);
    assert_eq!(facade.layer_count(), 3);
}

/// `CompileResult.stdlib_prelude` can be used to build facade.
#[test]
fn test_facade_from_compile_result() {
    let result = compile("fn main() {}");
    let facade = StdlibFacade::from_prelude(result.stdlib_prelude);
    assert!(facade.is_stdlib_name("i32"));
    assert!(facade.is_stdlib_name("Box"));
    assert!(facade.is_stdlib_name("File"));
}
