//! Stage 5.39: Stdlib vtable construction planner tests
//!
//! Tests `StdlibVtablePlanEntry` + `StdlibVtablePlan` +
//! `stdlib_vtable_plan()` + `stdlib_vtable_plan_entry_count()` +
//! `stdlib_vtable_plan_is_complete()` + `stdlib_vtable_plan_missing_methods()`.
//!
//! Per §16: tests use the public API only (no driver/hir/mir access).
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::stdlib::{
    stdlib_vtable_plan, stdlib_vtable_plan_entry_count, stdlib_vtable_plan_is_complete,
    stdlib_vtable_plan_missing_methods, StdlibVtablePlanEntry,
};

// ---------------------------------------------------------------------------
// stdlib_vtable_plan — basic construction
// ---------------------------------------------------------------------------

/// Clone with both methods provided → complete plan with 2 entries.
#[test]
fn test_stdlib_vtable_plan_clone_complete() {
    let plan =
        stdlib_vtable_plan("Clone", &["clone", "clone_from"]).expect("Clone should be registered");
    assert_eq!(plan.trait_name, "Clone");
    assert_eq!(plan.entries.len(), 2);

    assert_eq!(plan.entries[0].slot_index, 0);
    assert_eq!(plan.entries[0].method_name, "clone");
    assert!(plan.entries[0].provided);

    assert_eq!(plan.entries[1].slot_index, 1);
    assert_eq!(plan.entries[1].method_name, "clone_from");
    assert!(plan.entries[1].provided);
}

/// Clone with only `clone` provided → clone_from is missing.
#[test]
fn test_stdlib_vtable_plan_clone_partial() {
    let plan = stdlib_vtable_plan("Clone", &["clone"]).unwrap();
    assert_eq!(plan.entries.len(), 2);
    assert!(plan.entries[0].provided); // clone
    assert!(!plan.entries[1].provided); // clone_from
}

/// Drop with `drop` provided → 1 entry, complete.
#[test]
fn test_stdlib_vtable_plan_drop() {
    let plan = stdlib_vtable_plan("Drop", &["drop"]).unwrap();
    assert_eq!(plan.entries.len(), 1);
    assert_eq!(plan.entries[0].method_name, "drop");
    assert!(plan.entries[0].provided);
}

/// PartialEq with only `eq` → ne missing.
#[test]
fn test_stdlib_vtable_plan_partial_eq() {
    let plan = stdlib_vtable_plan("PartialEq", &["eq"]).unwrap();
    assert_eq!(plan.entries.len(), 2);
    assert!(plan.entries[0].provided); // eq
    assert!(!plan.entries[1].provided); // ne
}

/// Add with `add` provided → 1 entry, complete.
#[test]
fn test_stdlib_vtable_plan_add() {
    let plan = stdlib_vtable_plan("Add", &["add"]).unwrap();
    assert_eq!(plan.entries.len(), 1);
    assert!(plan.entries[0].provided);
    assert!(stdlib_vtable_plan_is_complete(&plan));
}

/// Marker trait (Copy) with empty provided → empty plan, vacuously complete.
#[test]
fn test_stdlib_vtable_plan_marker() {
    let plan = stdlib_vtable_plan("Copy", &[]).expect("Copy should be registered");
    assert_eq!(plan.trait_name, "Copy");
    assert!(plan.entries.is_empty());
    // Markers are vacuously complete (no slots to fill).
    assert!(stdlib_vtable_plan_is_complete(&plan));
    assert!(stdlib_vtable_plan_missing_methods(&plan).is_empty());
}

/// Unknown trait → None.
#[test]
fn test_stdlib_vtable_plan_unknown_trait() {
    assert_eq!(stdlib_vtable_plan("BogusTrait", &[]), None);
    assert_eq!(stdlib_vtable_plan("From", &["from"]), None); // not registered
    assert_eq!(stdlib_vtable_plan("", &[]), None);
}

/// Extra provided method names that don't match any trait method are
/// silently ignored — they don't add entries to the plan.
#[test]
fn test_stdlib_vtable_plan_extra_provided_ignored() {
    let plan = stdlib_vtable_plan("Clone", &["clone", "bogus", "another_extra"]).unwrap();
    assert_eq!(plan.entries.len(), 2); // still 2 (clone + clone_from)
    assert!(plan.entries[0].provided); // clone
    assert!(!plan.entries[1].provided); // clone_from NOT in provided list
}

// ---------------------------------------------------------------------------
// stdlib_vtable_plan_entry_count
// ---------------------------------------------------------------------------

/// `stdlib_vtable_plan_entry_count` matches slot count.
#[test]
fn test_stdlib_vtable_plan_entry_count() {
    assert_eq!(stdlib_vtable_plan_entry_count("Clone"), Some(2));
    assert_eq!(stdlib_vtable_plan_entry_count("Drop"), Some(1));
    assert_eq!(stdlib_vtable_plan_entry_count("PartialEq"), Some(2));
    assert_eq!(stdlib_vtable_plan_entry_count("Add"), Some(1));
    assert_eq!(stdlib_vtable_plan_entry_count("Copy"), Some(0)); // marker
    assert_eq!(stdlib_vtable_plan_entry_count("BogusTrait"), None);
}

// ---------------------------------------------------------------------------
// stdlib_vtable_plan_is_complete
// ---------------------------------------------------------------------------

/// Complete plan → is_complete returns true.
#[test]
fn test_stdlib_vtable_plan_is_complete_true() {
    let plan = stdlib_vtable_plan("Clone", &["clone", "clone_from"]).unwrap();
    assert!(stdlib_vtable_plan_is_complete(&plan));
    // Also test method form
    assert!(plan.is_complete());
}

/// Partial plan → is_complete returns false.
#[test]
fn test_stdlib_vtable_plan_is_complete_false() {
    let plan = stdlib_vtable_plan("Clone", &["clone"]).unwrap();
    assert!(!stdlib_vtable_plan_is_complete(&plan));
    // PartialEq with neither method
    let plan2 = stdlib_vtable_plan("PartialEq", &[]).unwrap();
    assert!(!stdlib_vtable_plan_is_complete(&plan2));
}

// ---------------------------------------------------------------------------
// stdlib_vtable_plan_missing_methods
// ---------------------------------------------------------------------------

/// Complete plan → missing_methods is empty.
#[test]
fn test_stdlib_vtable_plan_missing_methods_empty() {
    let plan = stdlib_vtable_plan("Clone", &["clone", "clone_from"]).unwrap();
    let missing = stdlib_vtable_plan_missing_methods(&plan);
    assert!(missing.is_empty());
}

/// Partial Clone plan → missing_methods returns ["clone_from"].
#[test]
fn test_stdlib_vtable_plan_missing_methods_partial() {
    let plan = stdlib_vtable_plan("Clone", &["clone"]).unwrap();
    let missing = stdlib_vtable_plan_missing_methods(&plan);
    assert_eq!(missing, vec!["clone_from"]);
}

/// PartialEq with no methods provided → missing = ["eq", "ne"] (in slot order).
#[test]
fn test_stdlib_vtable_plan_missing_methods_all() {
    let plan = stdlib_vtable_plan("PartialEq", &[]).unwrap();
    let missing = stdlib_vtable_plan_missing_methods(&plan);
    assert_eq!(missing, vec!["eq", "ne"]);
}

// ---------------------------------------------------------------------------
// Determinism + struct semantics
// ---------------------------------------------------------------------------

/// Repeated calls return identical plans (deterministic order).
#[test]
fn test_stdlib_vtable_plan_deterministic_order() {
    let plan1 = stdlib_vtable_plan("Clone", &["clone", "clone_from"]).unwrap();
    let plan2 = stdlib_vtable_plan("Clone", &["clone", "clone_from"]).unwrap();
    assert_eq!(plan1, plan2);
}

/// `StdlibVtablePlan` derives PartialEq/Eq.
#[test]
fn test_stdlib_vtable_plan_eq() {
    let plan1 = stdlib_vtable_plan("Drop", &["drop"]).unwrap();
    let plan2 = stdlib_vtable_plan("Drop", &["drop"]).unwrap();
    assert_eq!(plan1, plan2);

    let plan3 = stdlib_vtable_plan("Drop", &[]).unwrap();
    assert_ne!(plan1, plan3); // different `provided` flag
}

/// `StdlibVtablePlanEntry` field access works.
#[test]
fn test_stdlib_vtable_plan_entry_struct() {
    let entry = StdlibVtablePlanEntry {
        slot_index: 0,
        method_name: "clone",
        provided: true,
    };
    assert_eq!(entry.slot_index, 0);
    assert_eq!(entry.method_name, "clone");
    assert!(entry.provided);
}

/// Plan entries are ordered by slot_index ascending.
#[test]
fn test_stdlib_vtable_plan_entries_ordered_by_slot() {
    let plan = stdlib_vtable_plan("PartialEq", &["eq", "ne"]).unwrap();
    for (i, entry) in plan.entries.iter().enumerate() {
        assert_eq!(entry.slot_index as usize, i);
    }
}
