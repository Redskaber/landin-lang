//! Stage 5.75: find_dyn_trait_method_call_in_plan tests
//!
//! Tests `find_dyn_trait_method_call_in_plan()` — single-point lookup of a
//! `DynTraitMethodCall` in a `DynTraitMIRPlan` by (trait_name, type_name,
//! method_name).
//!
//! This is the FIRST query API on DynTraitMIRPlan — all prior APIs (5.61-5.74)
//! were whole-plan builders / emitters. Stage 5.75 enables `mir/lower/` to
//! look up the specific method call representation when lowering a HIR
//! `receiver.method(args)` expression whose receiver has `dyn Trait` type.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::{
    build_dyn_trait_mir_plan, find_dyn_trait_method_call_in_plan, DynTraitFatPtr,
    DynTraitMethodCall,
};

/// Empty plan — any query returns None.
#[test]
fn test_find_in_empty_plan_returns_none() {
    let plan = build_dyn_trait_mir_plan(&[], &[]);
    let found = find_dyn_trait_method_call_in_plan(&plan, "Drop", "S", "drop");
    assert!(found.is_none());
}

/// Single method call — exact match returns Some.
#[test]
fn test_find_single_exact_match() {
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = [DynTraitMethodCall::new("Drop", "S", "drop", 0, 0)];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan(&plan, "Drop", "S", "drop");
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.trait_name, "Drop");
    assert_eq!(found.type_name, "S");
    assert_eq!(found.method_name, "drop");
    assert_eq!(found.slot_index, 0);
    assert_eq!(found.param_count, 0);
}

/// Single method call — trait_name mismatch returns None.
#[test]
fn test_find_single_trait_mismatch() {
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = [DynTraitMethodCall::new("Drop", "S", "drop", 0, 0)];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan(&plan, "Clone", "S", "drop");
    assert!(found.is_none());
}

/// Single method call — type_name mismatch returns None.
#[test]
fn test_find_single_type_mismatch() {
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = [DynTraitMethodCall::new("Drop", "S", "drop", 0, 0)];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan(&plan, "Drop", "T", "drop");
    assert!(found.is_none());
}

/// Single method call — method_name mismatch returns None.
#[test]
fn test_find_single_method_mismatch() {
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = [DynTraitMethodCall::new("Drop", "S", "drop", 0, 0)];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan(&plan, "Drop", "S", "clone");
    assert!(found.is_none());
}

/// Multiple method calls — match the second entry.
#[test]
fn test_find_multiple_match_second() {
    let fps = [DynTraitFatPtr::new("Clone", "S")];
    let calls = [
        DynTraitMethodCall::new("Clone", "S", "clone", 0, 0),
        DynTraitMethodCall::new("Clone", "S", "clone_from", 1, 1),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan(&plan, "Clone", "S", "clone_from");
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.method_name, "clone_from");
    assert_eq!(found.slot_index, 1);
    assert_eq!(found.param_count, 1);
}

/// Multiple method calls — match the last entry.
#[test]
fn test_find_multiple_match_last() {
    let fps = [
        DynTraitFatPtr::new("Drop", "S"),
        DynTraitFatPtr::new("Display", "S"),
    ];
    let calls = [
        DynTraitMethodCall::new("Drop", "S", "drop", 0, 0),
        DynTraitMethodCall::new("Display", "S", "fmt", 0, 1),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan(&plan, "Display", "S", "fmt");
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.trait_name, "Display");
    assert_eq!(found.method_name, "fmt");
    assert_eq!(found.slot_index, 0);
    assert_eq!(found.param_count, 1);
}

/// Multiple method calls — no match returns None.
#[test]
fn test_find_multiple_no_match() {
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = [
        DynTraitMethodCall::new("Drop", "S", "drop", 0, 0),
        DynTraitMethodCall::new("Drop", "S", "finalize", 1, 0),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan(&plan, "Drop", "S", "nonexistent");
    assert!(found.is_none());
}

/// Case sensitivity: "Display" != "display".
#[test]
fn test_find_case_sensitive() {
    let fps = [DynTraitFatPtr::new("Display", "Vec")];
    let calls = [DynTraitMethodCall::new("Display", "Vec", "fmt", 0, 1)];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    // Correct case
    assert!(find_dyn_trait_method_call_in_plan(&plan, "Display", "Vec", "fmt").is_some());
    // Wrong case on trait_name
    assert!(find_dyn_trait_method_call_in_plan(&plan, "display", "Vec", "fmt").is_none());
    // Wrong case on type_name
    assert!(find_dyn_trait_method_call_in_plan(&plan, "Display", "vec", "fmt").is_none());
    // Wrong case on method_name
    assert!(find_dyn_trait_method_call_in_plan(&plan, "Display", "Vec", "FMT").is_none());
}

/// Same trait + type with multiple methods — distinguish by method_name.
#[test]
fn test_find_distinguishes_methods_same_trait_type() {
    let fps = [DynTraitFatPtr::new("Iterator", "Range")];
    let calls = [
        DynTraitMethodCall::new("Iterator", "Range", "next", 0, 0),
        DynTraitMethodCall::new("Iterator", "Range", "size_hint", 1, 0),
        DynTraitMethodCall::new("Iterator", "Range", "count", 2, 0),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);

    // Each method is independently findable.
    let next = find_dyn_trait_method_call_in_plan(&plan, "Iterator", "Range", "next");
    assert!(next.is_some());
    assert_eq!(next.unwrap().slot_index, 0);

    let size_hint = find_dyn_trait_method_call_in_plan(&plan, "Iterator", "Range", "size_hint");
    assert!(size_hint.is_some());
    assert_eq!(size_hint.unwrap().slot_index, 1);

    let count = find_dyn_trait_method_call_in_plan(&plan, "Iterator", "Range", "count");
    assert!(count.is_some());
    assert_eq!(count.unwrap().slot_index, 2);
}

/// Returned reference points to the correct entry in the plan.
#[test]
fn test_find_returns_correct_reference() {
    let fps = [
        DynTraitFatPtr::new("Drop", "A"),
        DynTraitFatPtr::new("Drop", "B"),
    ];
    let calls = [
        DynTraitMethodCall::new("Drop", "A", "drop", 0, 0),
        DynTraitMethodCall::new("Drop", "B", "drop", 0, 0),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);

    // Query for Drop.B::drop — must return the B entry, not A.
    let found = find_dyn_trait_method_call_in_plan(&plan, "Drop", "B", "drop");
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.type_name, "B");
    assert_eq!(found.trait_name, "Drop");
    assert_eq!(found.method_name, "drop");
}

/// No side effects — repeated calls return equivalent results.
#[test]
fn test_find_no_side_effects() {
    let fps = [DynTraitFatPtr::new("Foo", "S")];
    let calls = [DynTraitMethodCall::new("Foo", "S", "bar", 0, 0)];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let r1 = find_dyn_trait_method_call_in_plan(&plan, "Foo", "S", "bar");
    let r2 = find_dyn_trait_method_call_in_plan(&plan, "Foo", "S", "bar");
    assert!(r1.is_some());
    assert!(r2.is_some());
    assert_eq!(r1.unwrap().method_name, r2.unwrap().method_name);
    assert_eq!(r1.unwrap().slot_index, r2.unwrap().slot_index);
}
