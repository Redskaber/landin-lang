//! Stage 5.77: find_dyn_trait_method_call_in_plan_by_method tests
//!
//! Tests `find_dyn_trait_method_call_in_plan_by_method()` — fuzzy lookup
//! of a `DynTraitMethodCall` in a `DynTraitMIRPlan` by `method_name` only.
//!
//! This is Stage 5.75's fuzzy partner — when the caller knows only the
//! method name (not the trait/type), use this variant. Stage 5.78+ will
//! use it in `mir/lower/`'s `HirExprKind::MethodCall` branch where only
//! `method.name` is available from HIR.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::{
    build_dyn_trait_mir_plan, find_dyn_trait_method_call_in_plan,
    find_dyn_trait_method_call_in_plan_by_method, DynTraitFatPtr, DynTraitMethodCall,
};
use landin_compiler::stdlib::StdlibTypeKind;

/// Empty plan — any method query returns None.
#[test]
fn test_find_by_method_empty_plan_returns_none() {
    let plan = build_dyn_trait_mir_plan(&[], &[]);
    let found = find_dyn_trait_method_call_in_plan_by_method(&plan, "drop");
    assert!(found.is_none());
}

/// Single method call — exact method match returns Some.
#[test]
fn test_find_by_method_single_exact_match() {
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
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan_by_method(&plan, "drop");
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.method_name, "drop");
    assert_eq!(found.trait_name, "Drop");
    assert_eq!(found.type_name, "S");
    assert_eq!(found.slot_index, 0);
}

/// Single method call — method mismatch returns None.
#[test]
fn test_find_by_method_single_method_mismatch() {
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
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan_by_method(&plan, "clone");
    assert!(found.is_none());
}

/// Multiple calls — match the first entry.
#[test]
fn test_find_by_method_multiple_match_first() {
    let fps = [
        DynTraitFatPtr::new("Drop", "A"),
        DynTraitFatPtr::new("Drop", "B"),
    ];
    let calls = [
        DynTraitMethodCall::new("Drop", "A", "drop", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new("Drop", "B", "drop", 0, 0, StdlibTypeKind::Unit, vec![]),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan_by_method(&plan, "drop");
    assert!(found.is_some());
    // First-match-wins: should return the "A" entry.
    let found = found.unwrap();
    assert_eq!(found.type_name, "A");
    assert_eq!(found.trait_name, "Drop");
    assert_eq!(found.method_name, "drop");
}

/// Multiple calls — match a middle entry (non-drop method).
#[test]
fn test_find_by_method_multiple_match_middle() {
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
        DynTraitMethodCall::new(
            "Clone",
            "S",
            "clone_into",
            2,
            1,
            StdlibTypeKind::Unit,
            vec![],
        ),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan_by_method(&plan, "clone_from");
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.method_name, "clone_from");
    assert_eq!(found.slot_index, 1);
    assert_eq!(found.param_count, 1);
}

/// Multiple calls — match the last entry.
#[test]
fn test_find_by_method_multiple_match_last() {
    let fps = [
        DynTraitFatPtr::new("Drop", "S"),
        DynTraitFatPtr::new("Display", "S"),
    ];
    let calls = [
        DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new("Display", "S", "fmt", 0, 1, StdlibTypeKind::Unit, vec![]),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan_by_method(&plan, "fmt");
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.method_name, "fmt");
    assert_eq!(found.trait_name, "Display");
    assert_eq!(found.param_count, 1);
}

/// Multiple calls — no method matches returns None.
#[test]
fn test_find_by_method_multiple_no_match() {
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = [
        DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new("Drop", "S", "finalize", 1, 0, StdlibTypeKind::Unit, vec![]),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan_by_method(&plan, "nonexistent");
    assert!(found.is_none());
}

/// Case sensitivity: "drop" != "Drop".
#[test]
fn test_find_by_method_case_sensitive() {
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
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    // Lowercase match
    assert!(find_dyn_trait_method_call_in_plan_by_method(&plan, "drop").is_some());
    // Capitalized mismatch
    assert!(find_dyn_trait_method_call_in_plan_by_method(&plan, "Drop").is_none());
    // All-caps mismatch
    assert!(find_dyn_trait_method_call_in_plan_by_method(&plan, "DROP").is_none());
}

/// Same method_name across multiple traits — first-match-wins.
#[test]
fn test_find_by_method_same_name_across_traits() {
    let fps = [
        DynTraitFatPtr::new("TraitA", "S"),
        DynTraitFatPtr::new("TraitB", "S"),
    ];
    let calls = [
        DynTraitMethodCall::new("TraitA", "S", "execute", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new("TraitB", "S", "execute", 0, 0, StdlibTypeKind::Unit, vec![]),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan_by_method(&plan, "execute");
    assert!(found.is_some());
    // First-match-wins: should be TraitA.
    assert_eq!(found.unwrap().trait_name, "TraitA");
}

/// Same method_name across multiple types — first-match-wins.
#[test]
fn test_find_by_method_same_name_across_types() {
    let fps = [
        DynTraitFatPtr::new("Drop", "TypeA"),
        DynTraitFatPtr::new("Drop", "TypeB"),
    ];
    let calls = [
        DynTraitMethodCall::new("Drop", "TypeA", "drop", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new("Drop", "TypeB", "drop", 0, 0, StdlibTypeKind::Unit, vec![]),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let found = find_dyn_trait_method_call_in_plan_by_method(&plan, "drop");
    assert!(found.is_some());
    // First-match-wins: should be TypeA.
    assert_eq!(found.unwrap().type_name, "TypeA");
}

/// Consistency with 5.75's exact-lookup: when (trait, type, method) is
/// unique, both lookups return the same entry.
#[test]
fn test_fuzzy_and_exact_consistent_when_unique() {
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
    let plan = build_dyn_trait_mir_plan(&fps, &calls);

    let fuzzy = find_dyn_trait_method_call_in_plan_by_method(&plan, "drop");
    let exact = find_dyn_trait_method_call_in_plan(&plan, "Drop", "S", "drop");

    assert!(fuzzy.is_some());
    assert!(exact.is_some());
    // Both should point to the same entry — compare by content.
    assert_eq!(fuzzy.unwrap().method_name, exact.unwrap().method_name);
    assert_eq!(fuzzy.unwrap().trait_name, exact.unwrap().trait_name);
    assert_eq!(fuzzy.unwrap().type_name, exact.unwrap().type_name);
    assert_eq!(fuzzy.unwrap().slot_index, exact.unwrap().slot_index);
}

/// No side effects — repeated calls return equivalent results.
#[test]
fn test_find_by_method_no_side_effects() {
    let fps = [DynTraitFatPtr::new("Foo", "S")];
    let calls = [DynTraitMethodCall::new(
        "Foo",
        "S",
        "bar",
        0,
        0,
        StdlibTypeKind::Unit,
        vec![],
    )];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let r1 = find_dyn_trait_method_call_in_plan_by_method(&plan, "bar");
    let r2 = find_dyn_trait_method_call_in_plan_by_method(&plan, "bar");
    assert!(r1.is_some());
    assert!(r2.is_some());
    assert_eq!(r1.unwrap().method_name, r2.unwrap().method_name);
    assert_eq!(r1.unwrap().slot_index, r2.unwrap().slot_index);
}
