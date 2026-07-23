//! Stage 5.29: Stdlib layer query tests
//!
//! Tests `StdlibLayer` enum, `layer_for_name()`, and `names_for_layer()`.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;
use landin_compiler::stdlib::StdlibLayer;

/// `layer_for_name` should return Core for primitive types.
#[test]
fn test_layer_for_name_core() {
    let result = compile("fn main() {}");
    assert_eq!(
        result.stdlib_prelude.layer_for_name("i32"),
        StdlibLayer::Core
    );
    assert_eq!(
        result.stdlib_prelude.layer_for_name("bool"),
        StdlibLayer::Core
    );
    assert_eq!(
        result.stdlib_prelude.layer_for_name("str"),
        StdlibLayer::Core
    );
}

/// `layer_for_name` should return Alloc for heap types.
#[test]
fn test_layer_for_name_alloc() {
    let result = compile("fn main() {}");
    assert_eq!(
        result.stdlib_prelude.layer_for_name("Box"),
        StdlibLayer::Alloc
    );
    assert_eq!(
        result.stdlib_prelude.layer_for_name("Vec"),
        StdlibLayer::Alloc
    );
    assert_eq!(
        result.stdlib_prelude.layer_for_name("String"),
        StdlibLayer::Alloc
    );
}

/// `layer_for_name` should return None for unknown names.
#[test]
fn test_layer_for_name_none() {
    let result = compile("fn main() {}");
    assert_eq!(
        result.stdlib_prelude.layer_for_name("MyType"),
        StdlibLayer::None
    );
    assert_eq!(result.stdlib_prelude.layer_for_name(""), StdlibLayer::None);
}

/// `names_for_layer` should return core types for Core layer.
#[test]
fn test_names_for_layer_core() {
    let result = compile("fn main() {}");
    let core_names = result.stdlib_prelude.names_for_layer(StdlibLayer::Core);
    assert!(core_names.contains(&"i32"));
    assert!(core_names.contains(&"bool"));
    assert!(!core_names.contains(&"Box"));
}

/// `names_for_layer` should return alloc types for Alloc layer.
#[test]
fn test_names_for_layer_alloc() {
    let result = compile("fn main() {}");
    let alloc_names = result.stdlib_prelude.names_for_layer(StdlibLayer::Alloc);
    assert!(alloc_names.contains(&"Box"));
    assert!(alloc_names.contains(&"Vec"));
    assert!(!alloc_names.contains(&"i32"));
}

/// `names_for_layer` should return empty for None layer.
#[test]
fn test_names_for_layer_none() {
    let result = compile("fn main() {}");
    let none_names = result.stdlib_prelude.names_for_layer(StdlibLayer::None);
    assert!(none_names.is_empty());
}

/// `StdlibLayer` should support equality comparison.
#[test]
fn test_stdlib_layer_equality() {
    assert_eq!(StdlibLayer::Core, StdlibLayer::Core);
    assert_eq!(StdlibLayer::Alloc, StdlibLayer::Alloc);
    assert_ne!(StdlibLayer::Core, StdlibLayer::Alloc);
    assert_ne!(StdlibLayer::Core, StdlibLayer::None);
}
