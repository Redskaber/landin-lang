//! Stage 5.61: DynTraitFatPtr MIR-level representation tests
//!
//! Tests `DynTraitFatPtr` struct — MIR-level representation of a
//! `dyn Trait` fat pointer value. Foundation for dyn Trait MIR lowering.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::DynTraitFatPtr;

/// Constructor produces correct fields.
#[test]
fn test_dyn_trait_fat_ptr_new() {
    let fp = DynTraitFatPtr::new("Display", "MyType");
    assert_eq!(fp.trait_name, "Display");
    assert_eq!(fp.type_name, "MyType");
    assert_eq!(fp.data_symbol, ".data.MyType");
    assert_eq!(fp.vtable_symbol, ".vtable.Display.MyType");
    assert_eq!(fp.dynptr_symbol, ".dynptr.Display.MyType");
}

/// All fields are accessible.
#[test]
fn test_dyn_trait_fat_ptr_fields() {
    let fp = DynTraitFatPtr::new("Clone", "S");
    assert_eq!(fp.trait_name, "Clone");
    assert_eq!(fp.type_name, "S");
    assert_eq!(fp.data_symbol, ".data.S");
    assert_eq!(fp.vtable_symbol, ".vtable.Clone.S");
    assert_eq!(fp.dynptr_symbol, ".dynptr.Clone.S");
}

/// Non-marker trait → is_marker() returns false.
#[test]
fn test_dyn_trait_fat_ptr_is_marker_false() {
    let fp = DynTraitFatPtr::new("Clone", "S");
    assert!(!fp.is_marker());
    let fp2 = DynTraitFatPtr::new("Display", "Vec");
    assert!(!fp2.is_marker());
    let fp3 = DynTraitFatPtr::new("Drop", "S");
    assert!(!fp3.is_marker());
}

/// Marker traits → is_marker() returns true.
#[test]
fn test_dyn_trait_fat_ptr_is_marker_true() {
    for trait_name in &["Copy", "Send", "Sync", "Sized", "Unpin", "Eq"] {
        let fp = DynTraitFatPtr::new(trait_name, "S");
        assert!(fp.is_marker(), "{trait_name} should be a marker");
    }
}

/// PartialEq/Eq derived.
#[test]
fn test_dyn_trait_fat_ptr_eq() {
    let fp1 = DynTraitFatPtr::new("Clone", "S");
    let fp2 = DynTraitFatPtr::new("Clone", "S");
    assert_eq!(fp1, fp2);

    let fp3 = DynTraitFatPtr::new("Drop", "S");
    assert_ne!(fp1, fp3);
}

/// Clone derived.
#[test]
fn test_dyn_trait_fat_ptr_clone() {
    let fp1 = DynTraitFatPtr::new("Clone", "S");
    let fp2 = fp1.clone();
    assert_eq!(fp1, fp2);
}

/// Debug derived — can format.
#[test]
fn test_dyn_trait_fat_ptr_debug() {
    let fp = DynTraitFatPtr::new("Display", "Vec");
    let debug_str = format!("{fp:?}");
    assert!(debug_str.contains("Display"));
    assert!(debug_str.contains("Vec"));
}

/// Real scenario: multiple fat pointers for different (trait, type) pairs.
#[test]
fn test_dyn_trait_fat_ptr_real_scenario() {
    // S impls Clone + Drop + Display
    let fps = [
        DynTraitFatPtr::new("Clone", "S"),
        DynTraitFatPtr::new("Drop", "S"),
        DynTraitFatPtr::new("Display", "S"),
    ];

    // All share the same data_symbol (same type S)
    for fp in &fps {
        assert_eq!(fp.data_symbol, ".data.S");
    }

    // Each has a unique vtable_symbol + dynptr_symbol
    assert_eq!(fps[0].vtable_symbol, ".vtable.Clone.S");
    assert_eq!(fps[1].vtable_symbol, ".vtable.Drop.S");
    assert_eq!(fps[2].vtable_symbol, ".vtable.Display.S");

    assert_eq!(fps[0].dynptr_symbol, ".dynptr.Clone.S");
    assert_eq!(fps[1].dynptr_symbol, ".dynptr.Drop.S");
    assert_eq!(fps[2].dynptr_symbol, ".dynptr.Display.S");

    // None are markers
    for fp in &fps {
        assert!(!fp.is_marker());
    }
}

/// Multiple fat pointers with different types.
#[test]
fn test_dyn_trait_fat_ptr_multiple() {
    let cases = [("Display", "Vec"), ("Clone", "String"), ("Drop", "Box")];
    for (trait_name, type_name) in cases {
        let fp = DynTraitFatPtr::new(trait_name, type_name);
        assert_eq!(fp.trait_name, trait_name);
        assert_eq!(fp.type_name, type_name);
        assert_eq!(fp.data_symbol, format!(".data.{type_name}"));
        assert_eq!(
            fp.vtable_symbol,
            format!(".vtable.{trait_name}.{type_name}")
        );
        assert_eq!(
            fp.dynptr_symbol,
            format!(".dynptr.{trait_name}.{type_name}")
        );
    }
}
