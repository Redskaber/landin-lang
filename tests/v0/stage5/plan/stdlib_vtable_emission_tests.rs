//! Stage 5.41: Stdlib vtable emission plan (aggregate) tests
//!
//! Tests `StdlibVtableEmission` struct + `stdlib_vtable_emission()` +
//! `stdlib_vtable_emissions_for_traits()`.
//!
//! Per §16: tests use the public API only (no driver/hir/mir access).
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::stdlib::{
    stdlib_vtable_emission, stdlib_vtable_emissions_for_traits, StdlibVtableEmission,
};

// ---------------------------------------------------------------------------
// stdlib_vtable_emission — basic construction
// ---------------------------------------------------------------------------

/// Clone + S + [clone, clone_from] → 2 slots, complete, not marker.
#[test]
fn test_stdlib_vtable_emission_clone_complete() {
    let e = stdlib_vtable_emission("Clone", "S", &["clone", "clone_from"])
        .expect("Clone should be registered");
    assert_eq!(e.trait_name, "Clone");
    assert_eq!(e.type_name, "S");
    assert_eq!(e.global_name, ".vtable.Clone.S");
    assert_eq!(
        e.method_symbols,
        vec!["landin_Clone_S_clone", "landin_Clone_S_clone_from"]
    );
    assert_eq!(e.slot_count, 2);
    assert_eq!(e.byte_size_32, 8);
    assert_eq!(e.byte_size_64, 16);
    assert!(!e.is_marker);
    assert!(e.is_complete);
}

/// Clone + S + [clone] → 2 slots, NOT complete (clone_from missing).
#[test]
fn test_stdlib_vtable_emission_clone_partial() {
    let e = stdlib_vtable_emission("Clone", "S", &["clone"]).unwrap();
    assert_eq!(e.slot_count, 2);
    assert_eq!(e.method_symbols, vec!["landin_Clone_S_clone", "null"]);
    assert!(!e.is_complete);
    assert!(!e.is_marker);
}

/// Drop + S + [drop] → 1 slot, complete.
#[test]
fn test_stdlib_vtable_emission_drop() {
    let e = stdlib_vtable_emission("Drop", "S", &["drop"]).unwrap();
    assert_eq!(e.slot_count, 1);
    assert_eq!(e.method_symbols, vec!["landin_Drop_S_drop"]);
    assert_eq!(e.byte_size_32, 4);
    assert_eq!(e.byte_size_64, 8);
    assert!(e.is_complete);
    assert!(!e.is_marker);
}

/// Copy + S + [] → 0 slots, marker, vacuously complete.
#[test]
fn test_stdlib_vtable_emission_marker() {
    let e = stdlib_vtable_emission("Copy", "S", &[]).expect("Copy should be registered");
    assert_eq!(e.trait_name, "Copy");
    assert_eq!(e.slot_count, 0);
    assert!(e.method_symbols.is_empty());
    assert_eq!(e.byte_size_32, 0);
    assert_eq!(e.byte_size_64, 0);
    assert!(e.is_marker);
    assert!(e.is_complete); // vacuously — no slots to fill
}

/// Unknown trait → None.
#[test]
fn test_stdlib_vtable_emission_unknown_trait() {
    assert_eq!(stdlib_vtable_emission("BogusTrait", "S", &[]), None);
    assert_eq!(stdlib_vtable_emission("From", "S", &["from"]), None);
    assert_eq!(stdlib_vtable_emission("", "S", &[]), None);
}

// ---------------------------------------------------------------------------
// Field correctness
// ---------------------------------------------------------------------------

/// global_name field matches `.vtable.<trait>.<type>`.
#[test]
fn test_stdlib_vtable_emission_global_name() {
    let e = stdlib_vtable_emission("Display", "Vec", &["fmt"]).unwrap();
    assert_eq!(e.global_name, ".vtable.Display.Vec");
}

/// byte_size_32 / byte_size_64 fields correct for various slot counts.
#[test]
fn test_stdlib_vtable_emission_byte_sizes() {
    // Clone: 2 slots
    let e = stdlib_vtable_emission("Clone", "S", &["clone", "clone_from"]).unwrap();
    assert_eq!(e.byte_size_32, 8);
    assert_eq!(e.byte_size_64, 16);
    // PartialEq: 2 slots
    let e2 = stdlib_vtable_emission("PartialEq", "S", &["eq", "ne"]).unwrap();
    assert_eq!(e2.byte_size_32, 8);
    assert_eq!(e2.byte_size_64, 16);
    // Add: 1 slot
    let e3 = stdlib_vtable_emission("Add", "Vec", &["add"]).unwrap();
    assert_eq!(e3.byte_size_32, 4);
    assert_eq!(e3.byte_size_64, 8);
    // Copy: 0 slots
    let e4 = stdlib_vtable_emission("Copy", "S", &[]).unwrap();
    assert_eq!(e4.byte_size_32, 0);
    assert_eq!(e4.byte_size_64, 0);
}

/// is_complete true when all slots provided.
#[test]
fn test_stdlib_vtable_emission_is_complete_true() {
    let e = stdlib_vtable_emission("Clone", "S", &["clone", "clone_from"]).unwrap();
    assert!(e.is_complete);
    let e2 = stdlib_vtable_emission("Drop", "S", &["drop"]).unwrap();
    assert!(e2.is_complete);
}

/// is_complete false when some slots missing.
#[test]
fn test_stdlib_vtable_emission_is_complete_false() {
    let e = stdlib_vtable_emission("Clone", "S", &["clone"]).unwrap();
    assert!(!e.is_complete);
    let e2 = stdlib_vtable_emission("PartialEq", "S", &[]).unwrap();
    assert!(!e2.is_complete);
}

/// is_marker true for Copy/Send/Sync/Sized/Unpin/Eq, false for others.
#[test]
fn test_stdlib_vtable_emission_is_marker() {
    for trait_name in &["Copy", "Send", "Sync", "Sized", "Unpin", "Eq"] {
        let e = stdlib_vtable_emission(trait_name, "S", &[]).unwrap();
        assert!(e.is_marker, "{trait_name} should be a marker");
        assert_eq!(e.slot_count, 0);
    }
    // Non-markers
    let e = stdlib_vtable_emission("Clone", "S", &["clone", "clone_from"]).unwrap();
    assert!(!e.is_marker);
    let e2 = stdlib_vtable_emission("Add", "S", &["add"]).unwrap();
    assert!(!e2.is_marker);
}

/// Arith op: Add + Vec + [add] → 1 slot.
#[test]
fn test_stdlib_vtable_emission_arith() {
    let e = stdlib_vtable_emission("Add", "Vec", &["add"]).unwrap();
    assert_eq!(e.trait_name, "Add");
    assert_eq!(e.type_name, "Vec");
    assert_eq!(e.global_name, ".vtable.Add.Vec");
    assert_eq!(e.method_symbols, vec!["landin_Add_Vec_add"]);
    assert_eq!(e.slot_count, 1);
    assert!(e.is_complete);
    assert!(!e.is_marker);
}

// ---------------------------------------------------------------------------
// stdlib_vtable_emissions_for_traits (batch)
// ---------------------------------------------------------------------------

/// Batch: Clone + Drop on same type S → 2 emissions.
#[test]
fn test_stdlib_vtable_emissions_for_traits() {
    let emissions = stdlib_vtable_emissions_for_traits(
        &["Clone", "Drop"],
        "S",
        &["clone", "clone_from", "drop"],
    );
    assert_eq!(emissions.len(), 2);
    assert_eq!(emissions[0].trait_name, "Clone");
    assert_eq!(emissions[1].trait_name, "Drop");
    // Both should be complete (all methods provided)
    assert!(emissions[0].is_complete);
    assert!(emissions[1].is_complete);
}

/// Batch filters out unknown traits silently.
#[test]
fn test_stdlib_vtable_emissions_for_traits_filters_unknown() {
    let emissions = stdlib_vtable_emissions_for_traits(
        &["Clone", "BogusTrait", "Drop", "From"],
        "S",
        &["clone", "clone_from", "drop"],
    );
    // Only Clone + Drop should be returned (BogusTrait + From are not registered)
    assert_eq!(emissions.len(), 2);
    assert_eq!(emissions[0].trait_name, "Clone");
    assert_eq!(emissions[1].trait_name, "Drop");
}

/// Empty trait list → empty emissions.
#[test]
fn test_stdlib_vtable_emissions_for_traits_empty() {
    let emissions = stdlib_vtable_emissions_for_traits(&[], "S", &["clone"]);
    assert!(emissions.is_empty());
}

/// Batch with markers — markers are included (with is_marker=true).
#[test]
fn test_stdlib_vtable_emissions_for_traits_includes_markers() {
    let emissions =
        stdlib_vtable_emissions_for_traits(&["Clone", "Copy"], "S", &["clone", "clone_from"]);
    assert_eq!(emissions.len(), 2);
    assert_eq!(emissions[0].trait_name, "Clone");
    assert!(!emissions[0].is_marker);
    assert_eq!(emissions[1].trait_name, "Copy");
    assert!(emissions[1].is_marker);
}

// ---------------------------------------------------------------------------
// Struct semantics
// ---------------------------------------------------------------------------

/// StdlibVtableEmission derives PartialEq/Eq.
#[test]
fn test_stdlib_vtable_emission_struct_eq() {
    let e1 = stdlib_vtable_emission("Clone", "S", &["clone", "clone_from"]).unwrap();
    let e2 = stdlib_vtable_emission("Clone", "S", &["clone", "clone_from"]).unwrap();
    assert_eq!(e1, e2);

    let e3 = stdlib_vtable_emission("Clone", "S", &["clone"]).unwrap();
    assert_ne!(e1, e3); // different method_symbols / is_complete
}

/// StdlibVtableEmission field access works.
#[test]
fn test_stdlib_vtable_emission_struct_field_access() {
    let e: StdlibVtableEmission = stdlib_vtable_emission("Drop", "Vec", &["drop"]).unwrap();
    assert_eq!(e.trait_name, "Drop");
    assert_eq!(e.type_name, "Vec");
    assert_eq!(e.global_name, ".vtable.Drop.Vec");
    assert_eq!(e.method_symbols, vec!["landin_Drop_Vec_drop"]);
    assert_eq!(e.slot_count, 1);
    assert_eq!(e.byte_size_32, 4);
    assert_eq!(e.byte_size_64, 8);
    assert!(!e.is_marker);
    assert!(e.is_complete);
}
