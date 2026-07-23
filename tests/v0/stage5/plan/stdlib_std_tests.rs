//! Stage 5.30: Stdlib std layer tests
//!
//! Tests `STDLIB_STD_TYPES`, `STDLIB_STD_TRAITS`, `StdlibLayer::Std`,
//! and that std types/traits are registered + queryable.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;
use landin_compiler::stdlib::{all_stdlib_trait_names, all_stdlib_type_names, StdlibLayer};

/// `STDLIB_STD_TYPES` should contain OS-dependent types.
#[test]
fn test_std_types_present() {
    let types = all_stdlib_type_names();
    assert!(types.contains(&"File"));
    assert!(types.contains(&"Path"));
    assert!(types.contains(&"TcpStream"));
    assert!(types.contains(&"Thread"));
    assert!(types.contains(&"Mutex"));
    assert!(types.contains(&"Result"));
    assert!(types.contains(&"Option"));
}

/// `STDLIB_STD_TRAITS` should contain I/O traits.
#[test]
fn test_std_traits_present() {
    let traits = all_stdlib_trait_names();
    assert!(traits.contains(&"Read"));
    assert!(traits.contains(&"Write"));
    assert!(traits.contains(&"Seek"));
    assert!(traits.contains(&"Error"));
}

/// Std types should be interned after compile.
#[test]
fn test_std_types_interned() {
    let result = compile("fn main() {}");
    assert!(result.interner.get("File").is_some());
    assert!(result.interner.get("Path").is_some());
    assert!(result.interner.get("Result").is_some());
    assert!(result.interner.get("Option").is_some());
}

/// Std traits should be interned after compile.
#[test]
fn test_std_traits_interned() {
    let result = compile("fn main() {}");
    assert!(result.interner.get("Read").is_some());
    assert!(result.interner.get("Write").is_some());
}

/// `layer_for_name` should return Std for std-layer types.
#[test]
fn test_layer_for_name_std() {
    let result = compile("fn main() {}");
    assert_eq!(
        result.stdlib_prelude.layer_for_name("File"),
        StdlibLayer::Std
    );
    assert_eq!(
        result.stdlib_prelude.layer_for_name("Path"),
        StdlibLayer::Std
    );
    assert_eq!(
        result.stdlib_prelude.layer_for_name("Result"),
        StdlibLayer::Std
    );
    assert_eq!(
        result.stdlib_prelude.layer_for_name("Option"),
        StdlibLayer::Std
    );
}

/// `names_for_layer` should return std types for Std layer.
#[test]
fn test_names_for_layer_std() {
    let result = compile("fn main() {}");
    let std_names = result.stdlib_prelude.names_for_layer(StdlibLayer::Std);
    assert!(std_names.contains(&"File"));
    assert!(std_names.contains(&"Path"));
    assert!(!std_names.contains(&"i32"));
    assert!(!std_names.contains(&"Box"));
}

/// `StdlibLayer::Std` should be distinct from other layers.
#[test]
fn test_stdlib_layer_std_distinct() {
    assert_ne!(StdlibLayer::Std, StdlibLayer::Core);
    assert_ne!(StdlibLayer::Std, StdlibLayer::Alloc);
    assert_ne!(StdlibLayer::Std, StdlibLayer::None);
    assert_eq!(StdlibLayer::Std, StdlibLayer::Std);
}

/// Prelude should contain std types.
#[test]
fn test_prelude_contains_std() {
    let result = compile("fn main() {}");
    assert!(result.stdlib_prelude.contains("File"));
    assert!(result.stdlib_prelude.contains("Result"));
    assert!(result.stdlib_prelude.contains("Option"));
    assert!(result.stdlib_prelude.contains("Read"));
}
