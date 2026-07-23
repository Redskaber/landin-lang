//! Stage 5.53: Codegen trait-dispatch emission plan tests
//!
//! Tests `CodegenTraitDispatchEmissionPlan` struct +
//! `build_trait_dispatch_emission_plan()` — final aggregate API that
//! returns vtable_specs + dynptr_specs + summary in one call.
//!
//! **Critical invariant**: the plan's fields must match what the three
//! separate builder functions produce.
//! `test_build_trait_dispatch_emission_plan_match_separate_calls` verifies this.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{
    build_dynptr_global_specs, build_trait_dispatch_emission_plan,
    build_trait_dispatch_emission_summary, build_vtable_global_specs,
    CodegenTraitDispatchEmissionPlan,
};
use landin_compiler::traits::{TraitResolver, Vtable, VtableEntry};
use lasso::Rodeo;

// ---------------------------------------------------------------------------
// Helper: construct a TraitResolver with vtables for testing
// ---------------------------------------------------------------------------

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
            fn_name: sym.to_string(),
        })
        .collect();

    let vtable = Vtable {
        trait_name: trait_spur,
        self_ty_name: type_spur,
        impl_def_id: landin_compiler::hir::DefId::new(0),
        entries,
    };
    resolver.vtables.insert((trait_spur, type_spur), vtable);
    resolver
}

// ---------------------------------------------------------------------------
// Empty input
// ---------------------------------------------------------------------------

/// Empty TraitResolver → all empty/zero.
#[test]
fn test_build_trait_dispatch_emission_plan_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    assert!(plan.vtable_specs.is_empty());
    assert!(plan.dynptr_specs.is_empty());
    assert_eq!(plan.summary.vtable_count, 0);
    assert_eq!(plan.summary.dynptr_count, 0);
}

// ---------------------------------------------------------------------------
// Single vtable
// ---------------------------------------------------------------------------

/// Single vtable → 1 vtable spec + 1 dynptr spec + summary with 1 count.
#[test]
fn test_build_trait_dispatch_emission_plan_single() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    assert_eq!(plan.vtable_specs.len(), 1);
    assert_eq!(plan.dynptr_specs.len(), 1);
    assert_eq!(plan.summary.vtable_count, 1);
    assert_eq!(plan.summary.dynptr_count, 1);
    assert_eq!(plan.summary.total_global_count, 2);
}

// ---------------------------------------------------------------------------
// Multi vtable
// ---------------------------------------------------------------------------

/// Multiple vtables → multiple specs + summary.
#[test]
fn test_build_trait_dispatch_emission_plan_multi() {
    let mut interner = Rodeo::new();
    let mut resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let trait_spur = interner.get_or_intern("Bar");
    let type_spur = interner.get_or_intern("T");
    resolver.vtables.insert(
        (trait_spur, type_spur),
        Vtable {
            trait_name: trait_spur,
            self_ty_name: type_spur,
            impl_def_id: landin_compiler::hir::DefId::new(1),
            entries: vec![VtableEntry {
                method_name: interner.get_or_intern("baz"),
                fn_name: "landin_T_baz".to_string(),
            }],
        },
    );

    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    assert_eq!(plan.vtable_specs.len(), 2);
    assert_eq!(plan.dynptr_specs.len(), 2);
    assert_eq!(plan.summary.vtable_count, 2);
    assert_eq!(plan.summary.dynptr_count, 2);
    assert_eq!(plan.summary.total_global_count, 4);
}

// ---------------------------------------------------------------------------
// Field correctness
// ---------------------------------------------------------------------------

/// `vtable_specs` matches `build_vtable_global_specs()`.
#[test]
fn test_build_trait_dispatch_emission_plan_vtable_specs() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let separate = build_vtable_global_specs(&resolver, &interner);

    // Compare as sets (HashMap order may differ)
    assert_eq!(plan.vtable_specs.len(), separate.len());
    for spec in &separate {
        assert!(plan.vtable_specs.iter().any(|s| s == spec));
    }
}

/// `dynptr_specs` matches `build_dynptr_global_specs()`.
#[test]
fn test_build_trait_dispatch_emission_plan_dynptr_specs() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let separate = build_dynptr_global_specs(&resolver, &interner);

    assert_eq!(plan.dynptr_specs.len(), separate.len());
    for spec in &separate {
        assert!(plan.dynptr_specs.iter().any(|s| s == spec));
    }
}

/// `summary` matches `build_trait_dispatch_emission_summary()`.
#[test]
fn test_build_trait_dispatch_emission_plan_summary() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let separate = build_trait_dispatch_emission_summary(&resolver, &interner);

    assert_eq!(plan.summary, separate);
}

// ---------------------------------------------------------------------------
// **Critical**: matches separate calls
// ---------------------------------------------------------------------------

/// `build_trait_dispatch_emission_plan()` must produce fields identical to
/// calling the three separate builder functions.
#[test]
fn test_build_trait_dispatch_emission_plan_match_separate_calls() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_S_clone", "landin_S_clone_from"],
    );

    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);

    // Compare summary (deterministic)
    let separate_summary = build_trait_dispatch_emission_summary(&resolver, &interner);
    assert_eq!(plan.summary, separate_summary);

    // Compare vtable_specs as sets
    let separate_vtable = build_vtable_global_specs(&resolver, &interner);
    assert_eq!(plan.vtable_specs.len(), separate_vtable.len());
    for spec in &separate_vtable {
        assert!(plan.vtable_specs.iter().any(|s| s == spec));
    }

    // Compare dynptr_specs as sets
    let separate_dynptr = build_dynptr_global_specs(&resolver, &interner);
    assert_eq!(plan.dynptr_specs.len(), separate_dynptr.len());
    for spec in &separate_dynptr {
        assert!(plan.dynptr_specs.iter().any(|s| s == spec));
    }
}

// ---------------------------------------------------------------------------
// No side effects
// ---------------------------------------------------------------------------

/// Pure function — doesn't modify input TraitResolver.
#[test]
fn test_build_trait_dispatch_emission_plan_no_side_effects() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let vtables_count_before = resolver.vtables.len();

    let _plan = build_trait_dispatch_emission_plan(&resolver, &interner);

    assert_eq!(resolver.vtables.len(), vtables_count_before);
}

// ---------------------------------------------------------------------------
// Real scenario
// ---------------------------------------------------------------------------

/// Simulate real TraitResolver with multiple (trait, type) pairs.
#[test]
fn test_build_trait_dispatch_emission_plan_real_scenario() {
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::new();

    // S impls Clone + Drop + Display
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

    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    assert_eq!(plan.vtable_specs.len(), 3);
    assert_eq!(plan.dynptr_specs.len(), 3);
    assert_eq!(plan.summary.vtable_count, 3);
    assert_eq!(plan.summary.dynptr_count, 3);
    assert_eq!(plan.summary.total_global_count, 6);
    assert_eq!(plan.summary.total_method_slots, 4); // Clone(2)+Drop(1)+Display(1)
}

// ---------------------------------------------------------------------------
// Edge case: unresolved interner
// ---------------------------------------------------------------------------

/// interner.try_resolve fails → "Trait"/"Type" defaults.
#[test]
fn test_build_trait_dispatch_emission_plan_unresolved_interner() {
    let mut interner_with_spur = Rodeo::new();
    let trait_spur = interner_with_spur.get_or_intern("Foo");
    let type_spur = interner_with_spur.get_or_intern("S");

    let mut resolver = TraitResolver::new();
    resolver.vtables.insert(
        (trait_spur, type_spur),
        Vtable {
            trait_name: trait_spur,
            self_ty_name: type_spur,
            impl_def_id: landin_compiler::hir::DefId::new(0),
            entries: vec![],
        },
    );

    // Use a *fresh* interner that doesn't know about these Spurs.
    let fresh_interner = Rodeo::new();
    let plan = build_trait_dispatch_emission_plan(&resolver, &fresh_interner);

    assert_eq!(plan.vtable_specs.len(), 1);
    assert_eq!(plan.vtable_specs[0].global_name, ".vtable.Trait.Type");
    assert_eq!(plan.dynptr_specs.len(), 1);
    assert_eq!(plan.dynptr_specs[0].global_name, ".dynptr.Trait.Type");
    assert_eq!(plan.summary.trait_names, vec!["Trait".to_string()]);
    assert_eq!(plan.summary.type_names, vec!["Type".to_string()]);
}

// ---------------------------------------------------------------------------
// Struct semantics
// ---------------------------------------------------------------------------

/// `CodegenTraitDispatchEmissionPlan` derives PartialEq/Eq.
#[test]
fn test_build_trait_dispatch_emission_plan_struct_eq() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let plan1 = build_trait_dispatch_emission_plan(&resolver, &interner);
    let plan2 = build_trait_dispatch_emission_plan(&resolver, &interner);

    // summary is deterministic; specs may differ in HashMap order so compare
    // summary equality + spec length equality
    assert_eq!(plan1.summary, plan2.summary);
    assert_eq!(plan1.vtable_specs.len(), plan2.vtable_specs.len());
    assert_eq!(plan1.dynptr_specs.len(), plan2.dynptr_specs.len());
}

/// Field access works.
#[test]
fn test_build_trait_dispatch_emission_plan_field_access() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_S_drop"]);

    let plan: CodegenTraitDispatchEmissionPlan =
        build_trait_dispatch_emission_plan(&resolver, &interner);

    // vtable_specs field
    assert_eq!(plan.vtable_specs.len(), 1);
    assert_eq!(plan.vtable_specs[0].global_name, ".vtable.Drop.S");

    // dynptr_specs field
    assert_eq!(plan.dynptr_specs.len(), 1);
    assert_eq!(plan.dynptr_specs[0].global_name, ".dynptr.Drop.S");
    assert_eq!(plan.dynptr_specs[0].data_symbol, ".data.S");
    assert_eq!(plan.dynptr_specs[0].vtable_symbol, ".vtable.Drop.S");

    // summary field
    assert_eq!(plan.summary.vtable_count, 1);
    assert_eq!(plan.summary.dynptr_count, 1);
    assert_eq!(plan.summary.total_method_slots, 1);
}
