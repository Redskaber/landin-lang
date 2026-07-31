//! Stage 5.73: DynTraitMIRPlan tests
//!
//! Tests `DynTraitMIRPlan` struct + `build_dyn_trait_mir_plan()` +
//! `build_dyn_trait_mir_plan_from_resolver()`.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::{
    build_dyn_trait_mir_plan, build_dyn_trait_mir_plan_from_resolver, DynTraitFatPtr,
    DynTraitMethodCall,
};
use landin_compiler::stdlib::StdlibTypeKind;
use landin_compiler::traits::{TraitResolver, Vtable, VtableEntry};
use lasso::Rodeo;

fn make_resolver_with_vtable(
    interner: &mut Rodeo,
    trait_name: &str,
    type_name: &str,
    method_symbols: &[&str],
) -> TraitResolver {
    let mut resolver = TraitResolver::new();
    let trait_spur = interner.get_or_intern(trait_name);
    let type_spur = interner.get_or_intern(type_name);
    let entries: Vec<VtableEntry> = method_symbols
        .iter()
        .map(|&sym| VtableEntry {
            method_name: interner.get_or_intern(sym),
            fn_name: interner.get_or_intern(sym),
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
    resolver
}

/// Empty input → empty plan.
#[test]
fn test_mir_plan_empty() {
    let plan = build_dyn_trait_mir_plan(&[], &[]);
    assert!(plan.fat_ptrs.is_empty());
    assert!(plan.method_calls.is_empty());
    assert_eq!(plan.summary.fat_ptr_count, 0);
}

/// Single fat ptr + single method call.
#[test]
fn test_mir_plan_single() {
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
    assert_eq!(plan.fat_ptrs.len(), 1);
    assert_eq!(plan.method_calls.len(), 1);
    assert_eq!(plan.summary.fat_ptr_count, 1);
    assert_eq!(plan.summary.method_call_count, 1);
}

/// From resolver — Clone → 1 fat ptr + 2 method calls.
#[test]
fn test_mir_plan_from_resolver_clone() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Clone", "S", &["landin_S_clone"]);
    let plan = build_dyn_trait_mir_plan_from_resolver(&resolver, &interner);
    assert_eq!(plan.fat_ptrs.len(), 1);
    assert_eq!(plan.method_calls.len(), 2);
    assert_eq!(plan.summary.method_call_count, 2);
}

/// From resolver — empty.
#[test]
fn test_mir_plan_from_resolver_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let plan = build_dyn_trait_mir_plan_from_resolver(&resolver, &interner);
    assert!(plan.fat_ptrs.is_empty());
}

/// Summary fields correct.
#[test]
fn test_mir_plan_summary_fields() {
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
    let plan = build_dyn_trait_mir_plan(&fps, &calls);
    assert_eq!(plan.summary.fat_ptr_count, 1);
    assert_eq!(plan.summary.method_call_count, 2);
    assert_eq!(plan.summary.total_slots, 2);
}

/// PartialEq/Eq derived.
#[test]
fn test_mir_plan_eq() {
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
    let p1 = build_dyn_trait_mir_plan(&fps, &calls);
    let p2 = build_dyn_trait_mir_plan(&fps, &calls);
    assert_eq!(p1, p2);
}

/// Field access works.
#[test]
fn test_mir_plan_field_access() {
    let fps = [DynTraitFatPtr::new("Foo", "Bar")];
    let plan = build_dyn_trait_mir_plan(&fps, &[]);
    assert_eq!(plan.fat_ptrs[0].trait_name, "Foo");
    assert_eq!(plan.summary.trait_names, vec!["Foo".to_string()]);
}

/// Real scenario: S impls Clone + Drop + Display.
#[test]
fn test_mir_plan_real_scenario() {
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
                fn_name: interner.get_or_intern(m),
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
    assert_eq!(plan.fat_ptrs.len(), 3);
    assert_eq!(plan.method_calls.len(), 4);
    assert_eq!(plan.summary.fat_ptr_count, 3);
    assert_eq!(plan.summary.method_call_count, 4);
    assert_eq!(plan.summary.total_slots, 2);
    assert_eq!(plan.summary.trait_names.len(), 3);
    assert_eq!(plan.summary.type_names.len(), 1);
}

/// No side effects on resolver.
#[test]
fn test_mir_plan_no_side_effects() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Clone", "S", &["landin_S_clone"]);
    let count_before = resolver.vtables.len();
    let _ = build_dyn_trait_mir_plan_from_resolver(&resolver, &interner);
    assert_eq!(resolver.vtables.len(), count_before);
}
