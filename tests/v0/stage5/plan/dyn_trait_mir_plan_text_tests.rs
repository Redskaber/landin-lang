//! Stage 5.74: emit_dyn_trait_mir_plan_text tests
//!
//! Tests `emit_dyn_trait_mir_plan_text()` — converts DynTraitMIRPlan to
//! complete LLVM IR text (summary + fat ptrs + method calls).
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::{
    build_dyn_trait_mir_plan, build_dyn_trait_mir_plan_from_resolver, emit_dyn_trait_mir_plan_text,
    DynTraitFatPtr, DynTraitMethodCall,
};
use landin_compiler::stdlib::StdlibTypeKind;
use landin_compiler::traits::{TraitResolver, Vtable, VtableEntry};
use lasso::Rodeo;

/// Empty plan → summary comment only.
#[test]
fn test_plan_text_empty() {
    let plan = build_dyn_trait_mir_plan(&[], &[]);
    let text = emit_dyn_trait_mir_plan_text(&plan);
    assert!(text.contains("; DynTraitMIRSummary:"));
    assert!(text.contains("0 fat ptrs"));
}

/// Single fat ptr + single method call.
#[test]
fn test_plan_text_single() {
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = [DynTraitMethodCall::new(
        "Drop",
        "S",
        "drop",
        0,
        0,
        StdlibTypeKind::Unit,
    )];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let text = emit_dyn_trait_mir_plan_text(&plan);
    assert!(text.contains("; DynTraitMIRSummary: 1 fat ptrs, 1 method calls"));
    assert!(text.contains("@.dynptr.Drop.S"));
    assert!(text.contains("; dyn Drop.S::drop"));
}

/// Clone: 1 fat ptr + 2 method calls.
#[test]
fn test_plan_text_clone() {
    let fps = [DynTraitFatPtr::new("Clone", "S")];
    let calls = [
        DynTraitMethodCall::new("Clone", "S", "clone", 0, 0, StdlibTypeKind::Unit),
        DynTraitMethodCall::new("Clone", "S", "clone_from", 1, 1, StdlibTypeKind::Unit),
    ];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let text = emit_dyn_trait_mir_plan_text(&plan);
    assert!(text.contains("@.dynptr.Clone.S"));
    assert!(text.contains("; dyn Clone.S::clone"));
    assert!(text.contains("; dyn Clone.S::clone_from"));
}

/// From resolver — full IR generation.
#[test]
fn test_plan_text_from_resolver() {
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::new();
    let trait_spur = interner.get_or_intern("Drop");
    let type_spur = interner.get_or_intern("S");
    resolver.vtables.insert(
        (trait_spur, type_spur),
        Vtable {
            trait_name: trait_spur,
            self_ty_name: type_spur,
            impl_def_id: landin_compiler::hir::DefId::new(0),
            entries: vec![VtableEntry {
                method_name: interner.get_or_intern("drop"),
                fn_name: "landin_S_drop".to_string(),
            }],
        },
    );
    let plan = build_dyn_trait_mir_plan_from_resolver(&resolver, &interner);
    let text = emit_dyn_trait_mir_plan_text(&plan);
    assert!(text.contains("; DynTraitMIRSummary: 1 fat ptrs, 1 method calls"));
    assert!(text.contains("@.dynptr.Drop.S"));
    assert!(text.contains("; dyn Drop.S::drop"));
}

/// No side effects.
#[test]
fn test_plan_text_no_side_effects() {
    let fps = [DynTraitFatPtr::new("Foo", "S")];
    let plan = build_dyn_trait_mir_plan(&fps, &[]);
    let t1 = emit_dyn_trait_mir_plan_text(&plan);
    let t2 = emit_dyn_trait_mir_plan_text(&plan);
    assert_eq!(t1, t2);
}

/// Text contains both fat ptr globals and method call IR.
#[test]
fn test_plan_text_contains_both() {
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = [DynTraitMethodCall::new(
        "Drop",
        "S",
        "drop",
        0,
        0,
        StdlibTypeKind::Unit,
    )];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    let text = emit_dyn_trait_mir_plan_text(&plan);
    // Fat ptr global
    assert!(text.contains("private unnamed_addr constant"));
    // Method call
    assert!(text.contains("getelementptr"));
    assert!(text.contains("call ptr"));
}

/// Real scenario: Clone + Drop + Display.
#[test]
fn test_plan_text_real_scenario() {
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::new();
    for (trait_name, methods) in [
        ("Clone", vec!["landin_S_clone", "landin_S_clone_from"]),
        ("Drop", vec!["landin_S_drop"]),
        ("Display", vec!["landin_S_fmt"]),
    ] {
        let trait_spur = interner.get_or_intern(trait_name);
        let type_spur = interner.get_or_intern("S");
        let entries: Vec<VtableEntry> = methods
            .iter()
            .map(|&m| VtableEntry {
                method_name: interner.get_or_intern(m),
                fn_name: m.to_string(),
            })
            .collect();
        resolver.vtables.insert(
            (trait_spur, type_spur),
            Vtable {
                trait_name: trait_spur,
                self_ty_name: type_spur,
                impl_def_id: landin_compiler::hir::DefId::new(0),
                entries,
            },
        );
    }
    let plan = build_dyn_trait_mir_plan_from_resolver(&resolver, &interner);
    let text = emit_dyn_trait_mir_plan_text(&plan);
    assert!(text.contains("3 fat ptrs, 4 method calls"));
    assert!(text.contains("@.dynptr.Clone.S"));
    assert!(text.contains("@.dynptr.Drop.S"));
    assert!(text.contains("@.dynptr.Display.S"));
    assert!(text.contains("; dyn Clone.S::clone"));
    assert!(text.contains("; dyn Drop.S::drop"));
    assert!(text.contains("; dyn Display.S::fmt"));
}

/// Deterministic.
#[test]
fn test_plan_text_deterministic() {
    let fps = [DynTraitFatPtr::new("Foo", "S")];
    let plan = build_dyn_trait_mir_plan(&fps, &[]);
    let t1 = emit_dyn_trait_mir_plan_text(&plan);
    let t2 = emit_dyn_trait_mir_plan_text(&plan);
    assert_eq!(t1, t2);
}
