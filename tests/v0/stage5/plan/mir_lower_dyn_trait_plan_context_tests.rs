//! Stage 5.76: MirLowerCtxt dyn_trait_plan field + setter/getter tests
//!
//! Tests that `MirLowerCtxt` exposes the new `dyn_trait_plan: Option<DynTraitMIRPlan>`
//! field with `set_dyn_trait_plan()` setter and `dyn_trait_plan()` getter.
//! This is the **first mir/lower integration step** — context wiring only,
//! no lowering logic changes (those land in Stage 5.77+).
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::{
    build_dyn_trait_mir_plan, DynTraitFatPtr, DynTraitMethodCall, MirLowerCtxt,
};
use landin_compiler::session::Span;
use landin_compiler::stdlib::StdlibTypeKind;
use lasso::Rodeo;

/// Helper: build a non-trivial plan with 2 fat ptrs + 3 method calls.
fn build_sample_plan() -> landin_compiler::mir::DynTraitMIRPlan {
    let fps = [
        DynTraitFatPtr::new("Drop", "S"),
        DynTraitFatPtr::new("Clone", "T"),
    ];
    let calls = [
        DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit),
        DynTraitMethodCall::new("Clone", "T", "clone", 0, 0, StdlibTypeKind::Unit),
        DynTraitMethodCall::new("Clone", "T", "clone_from", 1, 1, StdlibTypeKind::Unit),
    ];
    build_dyn_trait_mir_plan(&fps, &calls)
}

/// new() does not attach a plan — getter returns None.
#[test]
fn test_new_cx_has_no_plan() {
    let interner = Rodeo::new();
    let cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    assert!(cx.dyn_trait_plan().is_none());
}

/// After set_dyn_trait_plan, getter returns Some.
#[test]
fn test_set_then_get_returns_some() {
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    let plan = build_sample_plan();
    cx.set_dyn_trait_plan(plan);
    assert!(cx.dyn_trait_plan().is_some());
}

/// Attached plan retains its fat_ptrs content.
#[test]
fn test_attached_plan_preserves_fat_ptrs() {
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    let plan = build_sample_plan();
    cx.set_dyn_trait_plan(plan);
    let got = cx.dyn_trait_plan().unwrap();
    assert_eq!(got.fat_ptrs.len(), 2);
    assert_eq!(got.fat_ptrs[0].trait_name, "Drop");
    assert_eq!(got.fat_ptrs[0].type_name, "S");
    assert_eq!(got.fat_ptrs[1].trait_name, "Clone");
    assert_eq!(got.fat_ptrs[1].type_name, "T");
}

/// Attached plan retains its method_calls content.
#[test]
fn test_attached_plan_preserves_method_calls() {
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    let plan = build_sample_plan();
    cx.set_dyn_trait_plan(plan);
    let got = cx.dyn_trait_plan().unwrap();
    assert_eq!(got.method_calls.len(), 3);
    assert_eq!(got.method_calls[0].method_name, "drop");
    assert_eq!(got.method_calls[1].method_name, "clone");
    assert_eq!(got.method_calls[2].method_name, "clone_from");
}

/// Attached plan retains its summary fields.
#[test]
fn test_attached_plan_preserves_summary() {
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    let plan = build_sample_plan();
    cx.set_dyn_trait_plan(plan);
    let got = cx.dyn_trait_plan().unwrap();
    assert_eq!(got.summary.fat_ptr_count, 2);
    assert_eq!(got.summary.method_call_count, 3);
}

/// Setting twice — last wins.
#[test]
fn test_set_twice_last_wins() {
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);

    let first = build_dyn_trait_mir_plan(&[DynTraitFatPtr::new("Drop", "A")], &[]);
    cx.set_dyn_trait_plan(first);

    let second = build_dyn_trait_mir_plan(
        &[DynTraitFatPtr::new("Clone", "B")],
        &[DynTraitMethodCall::new(
            "Clone",
            "B",
            "clone",
            0,
            0,
            StdlibTypeKind::Unit,
        )],
    );
    cx.set_dyn_trait_plan(second);

    let got = cx.dyn_trait_plan().unwrap();
    assert_eq!(got.fat_ptrs.len(), 1);
    assert_eq!(got.fat_ptrs[0].trait_name, "Clone");
    assert_eq!(got.fat_ptrs[0].type_name, "B");
    assert_eq!(got.method_calls.len(), 1);
    assert_eq!(got.method_calls[0].method_name, "clone");
}

/// Setting an empty plan is allowed — getter returns Some with empty fields.
#[test]
fn test_set_empty_plan_is_some() {
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    let plan = build_dyn_trait_mir_plan(&[], &[]);
    cx.set_dyn_trait_plan(plan);
    let got = cx.dyn_trait_plan().unwrap();
    assert!(got.fat_ptrs.is_empty());
    assert!(got.method_calls.is_empty());
    assert_eq!(got.summary.fat_ptr_count, 0);
    assert_eq!(got.summary.method_call_count, 0);
}

/// Setting a plan does not disturb other cx fields (mir / local_map / hir).
#[test]
fn test_set_plan_isolates_other_fields() {
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    let block_before = cx.current_block;
    let local_count_before = cx.mir.local_decls.len();
    // hir is None at construction (HirCrate does not impl PartialEq; we
    // verify its None-ness directly rather than via equality).
    assert!(cx.hir.is_none());

    let plan = build_sample_plan();
    cx.set_dyn_trait_plan(plan);

    assert_eq!(cx.current_block, block_before);
    assert_eq!(cx.mir.local_decls.len(), local_count_before);
    assert!(cx.hir.is_none());
}

/// Getter can be called repeatedly without side effects.
#[test]
fn test_getter_is_idempotent() {
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    cx.set_dyn_trait_plan(build_sample_plan());

    let g1 = cx.dyn_trait_plan();
    let g2 = cx.dyn_trait_plan();
    assert!(g1.is_some());
    assert!(g2.is_some());
    assert_eq!(
        g1.unwrap().method_calls.len(),
        g2.unwrap().method_calls.len()
    );
    assert_eq!(g1.unwrap().fat_ptrs.len(), g2.unwrap().fat_ptrs.len());
}

/// Empty plan via build_dyn_trait_mir_plan(&[], &[]) → set → get round-trip.
#[test]
fn test_round_trip_empty_plan() {
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    let plan = build_dyn_trait_mir_plan(&[], &[]);
    cx.set_dyn_trait_plan(plan);
    let got = cx.dyn_trait_plan().unwrap();
    assert!(got.fat_ptrs.is_empty());
    assert!(got.method_calls.is_empty());
}

/// Plan field is publicly accessible (pub field, not just via getter).
#[test]
fn test_dyn_trait_plan_field_is_pub() {
    let interner = Rodeo::new();
    let mut cx = MirLowerCtxt::new(&interner, Span::DUMMY);
    cx.set_dyn_trait_plan(build_sample_plan());
    // Direct field access (pub).
    assert!(cx.dyn_trait_plan.is_some());
    assert_eq!(cx.dyn_trait_plan.as_ref().unwrap().fat_ptrs.len(), 2);
}
