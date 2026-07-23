//! Stage 5.42: Stdlib vtable emission summary tests
//!
//! Tests `StdlibVtableEmissionSummary` struct +
//! `stdlib_vtable_emission_summary()`.
//!
//! Per §16: tests use the public API only (no driver/hir/mir access).
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::stdlib::{
    stdlib_vtable_emission, stdlib_vtable_emission_summary, stdlib_vtable_emissions_for_traits,
    StdlibVtableEmissionSummary,
};

// ---------------------------------------------------------------------------
// Empty input
// ---------------------------------------------------------------------------

/// Empty input → all-zero summary.
#[test]
fn test_stdlib_vtable_emission_summary_empty() {
    let s = stdlib_vtable_emission_summary(&[]);
    assert_eq!(s.total_emissions, 0);
    assert_eq!(s.marker_count, 0);
    assert_eq!(s.complete_count, 0);
    assert_eq!(s.incomplete_count, 0);
    assert_eq!(s.total_slots, 0);
    assert_eq!(s.total_byte_size_32, 0);
    assert_eq!(s.total_byte_size_64, 0);
    assert!(s.trait_names.is_empty());
}

// ---------------------------------------------------------------------------
// Single emission
// ---------------------------------------------------------------------------

/// Single complete emission → counts reflect that single emission.
#[test]
fn test_stdlib_vtable_emission_summary_single_complete() {
    let e = stdlib_vtable_emission("Clone", "S", &["clone", "clone_from"]).unwrap();
    let s = stdlib_vtable_emission_summary(&[e]);
    assert_eq!(s.total_emissions, 1);
    assert_eq!(s.marker_count, 0);
    assert_eq!(s.complete_count, 1);
    assert_eq!(s.incomplete_count, 0);
    assert_eq!(s.total_slots, 2);
    assert_eq!(s.total_byte_size_32, 8);
    assert_eq!(s.total_byte_size_64, 16);
    assert_eq!(s.trait_names, vec!["Clone"]);
}

/// Single marker emission → marker_count=1, complete_count=1 (vacuously).
#[test]
fn test_stdlib_vtable_emission_summary_single_marker() {
    let e = stdlib_vtable_emission("Copy", "S", &[]).unwrap();
    let s = stdlib_vtable_emission_summary(&[e]);
    assert_eq!(s.total_emissions, 1);
    assert_eq!(s.marker_count, 1);
    assert_eq!(s.complete_count, 1); // vacuously complete
    assert_eq!(s.incomplete_count, 0);
    assert_eq!(s.total_slots, 0);
    assert_eq!(s.total_byte_size_32, 0);
    assert_eq!(s.total_byte_size_64, 0);
    assert_eq!(s.trait_names, vec!["Copy"]);
}

// ---------------------------------------------------------------------------
// Multi-emission mixed
// ---------------------------------------------------------------------------

/// Multi-emission: Clone (complete) + Drop (complete) + Copy (marker) +
/// PartialEq (incomplete) → verify all counts.
#[test]
fn test_stdlib_vtable_emission_summary_multi_mixed() {
    let emissions = stdlib_vtable_emissions_for_traits(
        &["Clone", "Drop", "Copy", "PartialEq"],
        "S",
        &["clone", "clone_from", "drop", "eq"], // ne missing for PartialEq
    );
    assert_eq!(emissions.len(), 4);

    let s = stdlib_vtable_emission_summary(&emissions);
    assert_eq!(s.total_emissions, 4);
    assert_eq!(s.marker_count, 1); // Copy
    assert_eq!(s.complete_count, 3); // Clone + Drop + Copy (vacuous)
    assert_eq!(s.incomplete_count, 1); // PartialEq (ne missing)
                                       // Slots: Clone(2) + Drop(1) + Copy(0) + PartialEq(2) = 5
    assert_eq!(s.total_slots, 5);
    // 32-bit: 8 + 4 + 0 + 8 = 20
    assert_eq!(s.total_byte_size_32, 20);
    // 64-bit: 16 + 8 + 0 + 16 = 40
    assert_eq!(s.total_byte_size_64, 40);
}

// ---------------------------------------------------------------------------
// Field-specific tests
// ---------------------------------------------------------------------------

/// `total_slots` sums correctly across emissions.
#[test]
fn test_stdlib_vtable_emission_summary_total_slots() {
    let emissions = stdlib_vtable_emissions_for_traits(
        &["Clone", "PartialEq", "Drop"],
        "T",
        &["clone", "clone_from", "eq", "ne", "drop"],
    );
    let s = stdlib_vtable_emission_summary(&emissions);
    // Clone(2) + PartialEq(2) + Drop(1) = 5
    assert_eq!(s.total_slots, 5);
}

/// `total_byte_size_32` / `total_byte_size_64` sum correctly.
#[test]
fn test_stdlib_vtable_emission_summary_byte_sizes() {
    let emissions = stdlib_vtable_emissions_for_traits(
        &["Clone", "Add", "Drop"],
        "T",
        &["clone", "clone_from", "add", "drop"],
    );
    let s = stdlib_vtable_emission_summary(&emissions);
    // Clone(2 slots) + Add(1) + Drop(1) = 4 slots
    // 32-bit: 4 × 4 = 16; 64-bit: 4 × 8 = 32
    assert_eq!(s.total_slots, 4);
    assert_eq!(s.total_byte_size_32, 16);
    assert_eq!(s.total_byte_size_64, 32);
}

/// `trait_names` is deduplicated (same trait twice → one entry).
#[test]
fn test_stdlib_vtable_emission_summary_trait_names_dedup() {
    // Two emissions for the same trait (different types) → trait_names has
    // one entry.
    let e1 = stdlib_vtable_emission("Clone", "S1", &["clone", "clone_from"]).unwrap();
    let e2 = stdlib_vtable_emission("Clone", "S2", &["clone", "clone_from"]).unwrap();
    let s = stdlib_vtable_emission_summary(&[e1, e2]);
    assert_eq!(s.total_emissions, 2);
    assert_eq!(s.trait_names, vec!["Clone"]); // deduplicated
}

/// `trait_names` preserves first-seen order across multiple distinct traits.
#[test]
fn test_stdlib_vtable_emission_summary_trait_names_order() {
    let e1 = stdlib_vtable_emission("Drop", "S", &["drop"]).unwrap();
    let e2 = stdlib_vtable_emission("Clone", "S", &["clone", "clone_from"]).unwrap();
    let e3 = stdlib_vtable_emission("Drop", "T", &["drop"]).unwrap(); // dup
    let e4 = stdlib_vtable_emission("Add", "S", &["add"]).unwrap();
    let s = stdlib_vtable_emission_summary(&[e1, e2, e3, e4]);
    // First-seen order: Drop, Clone, Add (third Drop is dedup)
    assert_eq!(s.trait_names, vec!["Drop", "Clone", "Add"]);
}

/// `incomplete_count` counts emissions with missing methods.
#[test]
fn test_stdlib_vtable_emission_summary_incomplete_count() {
    let e1 = stdlib_vtable_emission("Clone", "S", &["clone"]).unwrap(); // incomplete
    let e2 = stdlib_vtable_emission("Clone", "T", &["clone", "clone_from"]).unwrap(); // complete
    let e3 = stdlib_vtable_emission("PartialEq", "S", &[]).unwrap(); // incomplete (both missing)
    let s = stdlib_vtable_emission_summary(&[e1, e2, e3]);
    assert_eq!(s.total_emissions, 3);
    assert_eq!(s.complete_count, 1);
    assert_eq!(s.incomplete_count, 2);
}

/// `marker_count` counts marker emissions.
#[test]
fn test_stdlib_vtable_emission_summary_marker_count() {
    let emissions = stdlib_vtable_emissions_for_traits(
        &["Copy", "Send", "Sync", "Clone"],
        "S",
        &["clone", "clone_from"],
    );
    let s = stdlib_vtable_emission_summary(&emissions);
    assert_eq!(s.total_emissions, 4);
    assert_eq!(s.marker_count, 3); // Copy + Send + Sync
    assert_eq!(s.complete_count, 4); // all (markers vacuously + Clone)
    assert_eq!(s.incomplete_count, 0);
}

/// `complete_count` counts only fully-provided emissions.
#[test]
fn test_stdlib_vtable_emission_summary_complete_count() {
    let e1 = stdlib_vtable_emission("Drop", "S", &["drop"]).unwrap(); // complete
    let e2 = stdlib_vtable_emission("Drop", "T", &[]).unwrap(); // incomplete
    let s = stdlib_vtable_emission_summary(&[e1, e2]);
    assert_eq!(s.complete_count, 1);
    assert_eq!(s.incomplete_count, 1);
}

// ---------------------------------------------------------------------------
// Struct semantics
// ---------------------------------------------------------------------------

/// `StdlibVtableEmissionSummary` derives PartialEq/Eq.
#[test]
fn test_stdlib_vtable_emission_summary_struct_eq() {
    let s1 = stdlib_vtable_emission_summary(&[stdlib_vtable_emission(
        "Clone",
        "S",
        &["clone", "clone_from"],
    )
    .unwrap()]);
    let s2 = stdlib_vtable_emission_summary(&[stdlib_vtable_emission(
        "Clone",
        "S",
        &["clone", "clone_from"],
    )
    .unwrap()]);
    assert_eq!(s1, s2);
}

/// `StdlibVtableEmissionSummary` field access works.
#[test]
fn test_stdlib_vtable_emission_summary_from_real_emissions() {
    let emissions = stdlib_vtable_emissions_for_traits(
        &["Clone", "Drop"],
        "MyType",
        &["clone", "clone_from", "drop"],
    );
    let s: StdlibVtableEmissionSummary = stdlib_vtable_emission_summary(&emissions);
    assert_eq!(s.total_emissions, 2);
    assert_eq!(s.trait_names, vec!["Clone", "Drop"]);
    assert!(s.incomplete_count == 0);
    assert!(s.complete_count == 2);
}
