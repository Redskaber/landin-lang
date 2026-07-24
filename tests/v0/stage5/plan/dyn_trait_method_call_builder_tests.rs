//! Stage 5.68: build_dyn_trait_method_calls_from_fat_ptrs tests
//!
//! Tests `build_dyn_trait_method_calls_from_fat_ptrs()` — bridge function
//! connecting stdlib trait method index with DynTraitMethodCall.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::{build_dyn_trait_method_calls_from_fat_ptrs, DynTraitFatPtr};

/// Empty input → empty Vec.
#[test]
fn test_build_method_calls_empty() {
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&[]);
    assert!(calls.is_empty());
}

/// Clone fat ptr → 2 method calls (clone@0, clone_from@1).
#[test]
fn test_build_method_calls_clone() {
    let fps = [DynTraitFatPtr::new("Clone", "S")];
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fps);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].method_name, "clone");
    assert_eq!(calls[0].slot_index, 0);
    assert_eq!(calls[1].method_name, "clone_from");
    assert_eq!(calls[1].slot_index, 1);
}

/// Drop fat ptr → 1 method call (drop@0).
#[test]
fn test_build_method_calls_drop() {
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fps);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method_name, "drop");
    assert_eq!(calls[0].slot_index, 0);
}

/// Marker trait (Copy) → 0 method calls.
#[test]
fn test_build_method_calls_marker() {
    let fps = [DynTraitFatPtr::new("Copy", "S")];
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fps);
    assert!(calls.is_empty());
}

/// Unregistered trait → 0 method calls (silently skipped).
#[test]
fn test_build_method_calls_unregistered() {
    let fps = [DynTraitFatPtr::new("BogusTrait", "S")];
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fps);
    assert!(calls.is_empty());
}

/// Multiple fat ptrs → combined method calls.
#[test]
fn test_build_method_calls_multi() {
    let fps = [
        DynTraitFatPtr::new("Clone", "S"),
        DynTraitFatPtr::new("Drop", "S"),
    ];
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fps);
    // Clone has 2 methods, Drop has 1 → 3 total
    assert_eq!(calls.len(), 3);
}

/// Method calls have correct trait/type names from fat ptr.
#[test]
fn test_build_method_calls_names() {
    let fps = [DynTraitFatPtr::new("Clone", "MyType")];
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fps);
    for call in &calls {
        assert_eq!(call.trait_name, "Clone");
        assert_eq!(call.type_name, "MyType");
    }
}

/// PartialEq → 2 method calls (eq@0, ne@1).
#[test]
fn test_build_method_calls_partial_eq() {
    let fps = [DynTraitFatPtr::new("PartialEq", "S")];
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fps);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].method_name, "eq");
    assert_eq!(calls[1].method_name, "ne");
}

/// Add trait → 1 method call (add@0).
#[test]
fn test_build_method_calls_add() {
    let fps = [DynTraitFatPtr::new("Add", "Vec")];
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fps);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method_name, "add");
    assert_eq!(calls[0].slot_index, 0);
}

/// Real scenario: S impls Clone + Drop + Display.
#[test]
fn test_build_method_calls_real_scenario() {
    let fps = [
        DynTraitFatPtr::new("Clone", "S"),
        DynTraitFatPtr::new("Drop", "S"),
        DynTraitFatPtr::new("Display", "S"),
    ];
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fps);
    // Clone(2) + Drop(1) + Display(1) = 4
    assert_eq!(calls.len(), 4);
    let method_names: Vec<&str> = calls.iter().map(|c| c.method_name.as_str()).collect();
    assert!(method_names.contains(&"clone"));
    assert!(method_names.contains(&"clone_from"));
    assert!(method_names.contains(&"drop"));
    assert!(method_names.contains(&"fmt"));
}
