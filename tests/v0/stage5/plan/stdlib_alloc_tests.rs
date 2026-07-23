//! Stage 5.28: Stdlib alloc layer tests
//!
//! Tests `STDLIB_ALLOC_TYPES`, `STDLIB_ALLOC_TRAITS`, and that alloc
//! types/traits are included in `all_stdlib_type_names()` /
//! `all_stdlib_trait_names()` / `register_stdlib()` / `StdlibPrelude`.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;
use landin_compiler::stdlib::{
    all_stdlib_trait_names, all_stdlib_type_names, STDLIB_ALLOC_TRAITS, STDLIB_ALLOC_TYPES,
};

/// `STDLIB_ALLOC_TYPES` should contain heap collection types.
#[test]
fn test_stdlib_alloc_types() {
    assert!(STDLIB_ALLOC_TYPES.contains(&"Box"));
    assert!(STDLIB_ALLOC_TYPES.contains(&"Vec"));
    assert!(STDLIB_ALLOC_TYPES.contains(&"String"));
    assert!(STDLIB_ALLOC_TYPES.contains(&"HashMap"));
    assert!(STDLIB_ALLOC_TYPES.contains(&"Rc"));
    assert!(STDLIB_ALLOC_TYPES.contains(&"Arc"));
}

/// `STDLIB_ALLOC_TRAITS` should contain formatting + smart-pointer traits.
#[test]
fn test_stdlib_alloc_traits() {
    assert!(STDLIB_ALLOC_TRAITS.contains(&"Display"));
    assert!(STDLIB_ALLOC_TRAITS.contains(&"Debug"));
    assert!(STDLIB_ALLOC_TRAITS.contains(&"Deref"));
    assert!(STDLIB_ALLOC_TRAITS.contains(&"Default"));
    assert!(STDLIB_ALLOC_TRAITS.contains(&"Hash"));
}

/// `all_stdlib_type_names()` should include alloc types.
#[test]
fn test_all_type_names_includes_alloc() {
    let names = all_stdlib_type_names();
    assert!(names.contains(&"Box"));
    assert!(names.contains(&"Vec"));
    assert!(names.contains(&"String"));
    assert!(names.contains(&"i32")); // core type still present
}

/// `all_stdlib_trait_names()` should include alloc traits.
#[test]
fn test_all_trait_names_includes_alloc() {
    let names = all_stdlib_trait_names();
    assert!(names.contains(&"Display"));
    assert!(names.contains(&"Debug"));
    assert!(names.contains(&"Deref"));
    assert!(names.contains(&"Add")); // ops trait still present
}

/// Alloc types should be interned after compile (via driver register_stdlib).
#[test]
fn test_alloc_types_interned() {
    let result = compile("fn main() {}");
    assert!(
        result.interner.get("Box").is_some(),
        "Box should be interned"
    );
    assert!(
        result.interner.get("Vec").is_some(),
        "Vec should be interned"
    );
    assert!(
        result.interner.get("String").is_some(),
        "String should be interned"
    );
    assert!(
        result.interner.get("HashMap").is_some(),
        "HashMap should be interned"
    );
}

/// Alloc traits should be interned after compile.
#[test]
fn test_alloc_traits_interned() {
    let result = compile("fn main() {}");
    assert!(
        result.interner.get("Display").is_some(),
        "Display should be interned"
    );
    assert!(
        result.interner.get("Debug").is_some(),
        "Debug should be interned"
    );
    assert!(
        result.interner.get("Deref").is_some(),
        "Deref should be interned"
    );
}

/// `StdlibPrelude` should contain alloc types + traits.
#[test]
fn test_prelude_contains_alloc() {
    let result = compile("fn main() {}");
    assert!(result.stdlib_prelude.contains("Box"));
    assert!(result.stdlib_prelude.contains("Vec"));
    assert!(result.stdlib_prelude.contains("Display"));
    assert!(result.stdlib_prelude.contains("Deref"));
}

/// Alloc type count should be 13.
#[test]
fn test_alloc_type_count() {
    assert_eq!(STDLIB_ALLOC_TYPES.len(), 13);
}

/// Alloc trait count should be 8.
#[test]
fn test_alloc_trait_count() {
    assert_eq!(STDLIB_ALLOC_TRAITS.len(), 8);
}
