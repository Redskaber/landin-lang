//! Stage 5.66: DynTraitMethodCall MIR representation tests
//!
//! Tests `DynTraitMethodCall` struct — MIR-level representation of a
//! `dyn Trait` method call.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::{DynTraitFatPtr, DynTraitMethodCall};
use landin_compiler::stdlib::StdlibTypeKind;

/// Constructor produces correct fields.
#[test]
fn test_dyn_trait_method_call_new() {
    let call = DynTraitMethodCall::new("Display", "Vec", "fmt", 0, 1, StdlibTypeKind::Unit);
    assert_eq!(call.trait_name, "Display");
    assert_eq!(call.type_name, "Vec");
    assert_eq!(call.method_name, "fmt");
    assert_eq!(call.slot_index, 0);
    assert_eq!(call.param_count, 1);
}

/// from_fat_ptr borrows trait/type from fat ptr.
#[test]
fn test_dyn_trait_method_call_from_fat_ptr() {
    let fp = DynTraitFatPtr::new("Clone", "S");
    let call = DynTraitMethodCall::from_fat_ptr(&fp, "clone", 0, 0, StdlibTypeKind::Unit);
    assert_eq!(call.trait_name, "Clone");
    assert_eq!(call.type_name, "S");
    assert_eq!(call.method_name, "clone");
    assert_eq!(call.slot_index, 0);
    assert_eq!(call.param_count, 0);
}

/// vtable_symbol returns correct format.
#[test]
fn test_dyn_trait_method_call_vtable_symbol() {
    let call = DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit);
    assert_eq!(call.vtable_symbol(), ".vtable.Drop.S");
}

/// dynptr_symbol returns correct format.
#[test]
fn test_dyn_trait_method_call_dynptr_symbol() {
    let call = DynTraitMethodCall::new("Display", "Vec", "fmt", 0, 1, StdlibTypeKind::Unit);
    assert_eq!(call.dynptr_symbol(), ".dynptr.Display.Vec");
}

/// PartialEq/Eq derived.
#[test]
fn test_dyn_trait_method_call_eq() {
    let c1 = DynTraitMethodCall::new("Clone", "S", "clone", 0, 0, StdlibTypeKind::Unit);
    let c2 = DynTraitMethodCall::new("Clone", "S", "clone", 0, 0, StdlibTypeKind::Unit);
    assert_eq!(c1, c2);

    let c3 = DynTraitMethodCall::new("Clone", "S", "clone_from", 1, 1, StdlibTypeKind::Unit);
    assert_ne!(c1, c3);
}

/// Clone derived.
#[test]
fn test_dyn_trait_method_call_clone() {
    let c1 = DynTraitMethodCall::new("Drop", "T", "drop", 0, 0, StdlibTypeKind::Unit);
    let c2 = c1.clone();
    assert_eq!(c1, c2);
}

/// Debug derived.
#[test]
fn test_dyn_trait_method_call_debug() {
    let call = DynTraitMethodCall::new("Display", "Vec", "fmt", 0, 1, StdlibTypeKind::Unit);
    let s = format!("{call:?}");
    assert!(s.contains("Display"));
    assert!(s.contains("fmt"));
}

/// Real scenario: Clone::clone call.
#[test]
fn test_dyn_trait_method_call_real_clone() {
    let fp = DynTraitFatPtr::new("Clone", "MyType");
    let call = DynTraitMethodCall::from_fat_ptr(&fp, "clone", 0, 0, StdlibTypeKind::Unit);
    assert_eq!(call.trait_name, "Clone");
    assert_eq!(call.type_name, "MyType");
    assert_eq!(call.method_name, "clone");
    assert_eq!(call.slot_index, 0);
    assert_eq!(call.param_count, 0);
    assert_eq!(call.vtable_symbol(), ".vtable.Clone.MyType");
    assert_eq!(call.dynptr_symbol(), ".dynptr.Clone.MyType");
}

/// Real scenario: Display::fmt call.
#[test]
fn test_dyn_trait_method_call_real_display() {
    let call = DynTraitMethodCall::new("Display", "Vec", "fmt", 0, 1, StdlibTypeKind::Unit);
    assert_eq!(call.slot_index, 0);
    assert_eq!(call.param_count, 1);
    assert_eq!(call.vtable_symbol(), ".vtable.Display.Vec");
}

/// Multiple method calls for same (trait, type) — different slots.
#[test]
fn test_dyn_trait_method_call_multiple_slots() {
    let fp = DynTraitFatPtr::new("Clone", "S");
    let call1 = DynTraitMethodCall::from_fat_ptr(&fp, "clone", 0, 0, StdlibTypeKind::Unit);
    let call2 = DynTraitMethodCall::from_fat_ptr(&fp, "clone_from", 1, 1, StdlibTypeKind::Unit);
    assert_eq!(call1.slot_index, 0);
    assert_eq!(call2.slot_index, 1);
    assert_ne!(call1, call2);
    // Same vtable for both
    assert_eq!(call1.vtable_symbol(), call2.vtable_symbol());
}
