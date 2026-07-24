//! Stage 5.71: DynTraitMIRSummary tests
//!
//! Tests `DynTraitMIRSummary` struct + `build_dyn_trait_mir_summary()`.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::{build_dyn_trait_mir_summary, DynTraitFatPtr, DynTraitMethodCall};
use landin_compiler::stdlib::StdlibTypeKind;

/// Empty input → all-zero summary.
#[test]
fn test_mir_summary_empty() {
    let s = build_dyn_trait_mir_summary(&[], &[]);
    assert_eq!(s.fat_ptr_count, 0);
    assert_eq!(s.method_call_count, 0);
    assert_eq!(s.total_slots, 0);
    assert!(s.trait_names.is_empty());
    assert!(s.type_names.is_empty());
}

/// Single fat ptr + single method call.
#[test]
fn test_mir_summary_single() {
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = [DynTraitMethodCall::new(
        "Drop",
        "S",
        "drop",
        0,
        0,
        StdlibTypeKind::Unit,
        vec![],
    )];
    let s = build_dyn_trait_mir_summary(&fps, &calls);
    assert_eq!(s.fat_ptr_count, 1);
    assert_eq!(s.method_call_count, 1);
    assert_eq!(s.total_slots, 1);
    assert_eq!(s.trait_names, vec!["Drop".to_string()]);
    assert_eq!(s.type_names, vec!["S".to_string()]);
}

/// Clone: 1 fat ptr + 2 method calls (slot 0 + slot 1).
#[test]
fn test_mir_summary_clone() {
    let fps = [DynTraitFatPtr::new("Clone", "S")];
    let calls = [
        DynTraitMethodCall::new("Clone", "S", "clone", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new(
            "Clone",
            "S",
            "clone_from",
            1,
            1,
            StdlibTypeKind::Unit,
            vec![],
        ),
    ];
    let s = build_dyn_trait_mir_summary(&fps, &calls);
    assert_eq!(s.fat_ptr_count, 1);
    assert_eq!(s.method_call_count, 2);
    assert_eq!(s.total_slots, 2); // max slot_index + 1 = 1 + 1 = 2
}

/// Multiple traits + types → deduplicated names.
#[test]
fn test_mir_summary_dedup() {
    let fps = [
        DynTraitFatPtr::new("Clone", "S"),
        DynTraitFatPtr::new("Drop", "S"),  // same type "S"
        DynTraitFatPtr::new("Clone", "T"), // same trait "Clone"
    ];
    let calls: Vec<DynTraitMethodCall> = vec![];
    let s = build_dyn_trait_mir_summary(&fps, &calls);
    assert_eq!(s.fat_ptr_count, 3);
    assert_eq!(s.trait_names.len(), 2); // Clone + Drop
    assert_eq!(s.type_names.len(), 2); // S + T
}

/// Total slots = max slot_index + 1.
#[test]
fn test_mir_summary_total_slots() {
    let fps = [DynTraitFatPtr::new("PartialEq", "S")];
    let calls = [
        DynTraitMethodCall::new("PartialEq", "S", "eq", 0, 1, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new("PartialEq", "S", "ne", 1, 1, StdlibTypeKind::Unit, vec![]),
    ];
    let s = build_dyn_trait_mir_summary(&fps, &calls);
    assert_eq!(s.total_slots, 2);
}

/// No method calls → total_slots = 0.
#[test]
fn test_mir_summary_no_calls() {
    let fps = [DynTraitFatPtr::new("Copy", "S")];
    let s = build_dyn_trait_mir_summary(&fps, &[]);
    assert_eq!(s.method_call_count, 0);
    assert_eq!(s.total_slots, 0);
}

/// PartialEq/Eq derived.
#[test]
fn test_mir_summary_eq() {
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = [DynTraitMethodCall::new(
        "Drop",
        "S",
        "drop",
        0,
        0,
        StdlibTypeKind::Unit,
        vec![],
    )];
    let s1 = build_dyn_trait_mir_summary(&fps, &calls);
    let s2 = build_dyn_trait_mir_summary(&fps, &calls);
    assert_eq!(s1, s2);
}

/// Real scenario: Clone + Drop + Display on S.
#[test]
fn test_mir_summary_real_scenario() {
    let fps = [
        DynTraitFatPtr::new("Clone", "S"),
        DynTraitFatPtr::new("Drop", "S"),
        DynTraitFatPtr::new("Display", "S"),
    ];
    let calls = [
        DynTraitMethodCall::new("Clone", "S", "clone", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new(
            "Clone",
            "S",
            "clone_from",
            1,
            1,
            StdlibTypeKind::Unit,
            vec![],
        ),
        DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new("Display", "S", "fmt", 0, 1, StdlibTypeKind::Unit, vec![]),
    ];
    let s = build_dyn_trait_mir_summary(&fps, &calls);
    assert_eq!(s.fat_ptr_count, 3);
    assert_eq!(s.method_call_count, 4);
    assert_eq!(s.total_slots, 2); // max slot is 1 (clone_from), so 1+1=2
    assert_eq!(s.trait_names.len(), 3);
    assert_eq!(s.type_names.len(), 1); // all "S"
}

/// Field access works.
#[test]
fn test_mir_summary_field_access() {
    let fps = [DynTraitFatPtr::new("Foo", "Bar")];
    let s = build_dyn_trait_mir_summary(&fps, &[]);
    assert_eq!(s.fat_ptr_count, 1);
    assert_eq!(s.trait_names, vec!["Foo".to_string()]);
    assert_eq!(s.type_names, vec!["Bar".to_string()]);
}
