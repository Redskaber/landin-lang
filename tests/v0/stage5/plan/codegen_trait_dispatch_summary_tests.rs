//! Stage 5.52: Codegen trait-dispatch emission summary tests
//!
//! Tests `CodegenTraitDispatchEmissionSummary` struct +
//! `build_trait_dispatch_emission_summary()` — pure free function that
//! computes project-level trait-dispatch emission statistics from
//! `TraitResolver.vtables`.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{
    build_trait_dispatch_emission_summary, CodegenTraitDispatchEmissionSummary,
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

/// Empty TraitResolver → all-zero summary.
#[test]
fn test_build_trait_dispatch_emission_summary_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let s = build_trait_dispatch_emission_summary(&resolver, &interner);
    assert_eq!(s.vtable_count, 0);
    assert_eq!(s.dynptr_count, 0);
    assert_eq!(s.total_global_count, 0);
    assert!(s.trait_names.is_empty());
    assert!(s.type_names.is_empty());
    assert_eq!(s.total_method_slots, 0);
}

// ---------------------------------------------------------------------------
// Single vtable
// ---------------------------------------------------------------------------

/// Single vtable → 1 vtable + 1 dynptr, 1 trait, 1 type.
#[test]
fn test_build_trait_dispatch_emission_summary_single() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let s = build_trait_dispatch_emission_summary(&resolver, &interner);
    assert_eq!(s.vtable_count, 1);
    assert_eq!(s.dynptr_count, 1);
    assert_eq!(s.total_global_count, 2);
    assert_eq!(s.trait_names, vec!["Foo".to_string()]);
    assert_eq!(s.type_names, vec!["S".to_string()]);
    assert_eq!(s.total_method_slots, 1);
}

// ---------------------------------------------------------------------------
// Multi vtable
// ---------------------------------------------------------------------------

/// Multiple vtables → counts and names reflect all.
#[test]
fn test_build_trait_dispatch_emission_summary_multi() {
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
            entries: vec![
                VtableEntry {
                    method_name: interner.get_or_intern("baz"),
                    fn_name: "landin_T_baz".to_string(),
                },
                VtableEntry {
                    method_name: interner.get_or_intern("qux"),
                    fn_name: "landin_T_qux".to_string(),
                },
            ],
        },
    );

    let s = build_trait_dispatch_emission_summary(&resolver, &interner);
    assert_eq!(s.vtable_count, 2);
    assert_eq!(s.dynptr_count, 2);
    assert_eq!(s.total_global_count, 4);
    assert_eq!(s.trait_names.len(), 2);
    assert!(s.trait_names.contains(&"Foo".to_string()));
    assert!(s.trait_names.contains(&"Bar".to_string()));
    assert_eq!(s.type_names.len(), 2);
    assert!(s.type_names.contains(&"S".to_string()));
    assert!(s.type_names.contains(&"T".to_string()));
    // Foo has 1 method, Bar has 2 methods → total 3
    assert_eq!(s.total_method_slots, 3);
}

// ---------------------------------------------------------------------------
// Field-specific tests
// ---------------------------------------------------------------------------

/// `vtable_count` matches vtables.len().
#[test]
fn test_build_trait_dispatch_emission_summary_vtable_count() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let s = build_trait_dispatch_emission_summary(&resolver, &interner);
    assert_eq!(s.vtable_count, 1);
}

/// `dynptr_count` == `vtable_count` (one dynptr per (trait, type) pair).
#[test]
fn test_build_trait_dispatch_emission_summary_dynptr_count() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let s = build_trait_dispatch_emission_summary(&resolver, &interner);
    assert_eq!(s.dynptr_count, s.vtable_count);
}

/// `total_global_count` == `vtable_count + dynptr_count`.
#[test]
fn test_build_trait_dispatch_emission_summary_total_global_count() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let s = build_trait_dispatch_emission_summary(&resolver, &interner);
    assert_eq!(s.total_global_count, s.vtable_count + s.dynptr_count);
    assert_eq!(s.total_global_count, 2);
}

/// `trait_names` deduplicated.
#[test]
fn test_build_trait_dispatch_emission_summary_trait_names_dedup() {
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::new();
    // Same trait (Clone) on two types (S, T)
    for type_name in ["S", "T"] {
        let trait_spur = interner.get_or_intern("Clone");
        let type_spur = interner.get_or_intern(type_name);
        resolver.vtables.insert(
            (trait_spur, type_spur),
            Vtable {
                trait_name: trait_spur,
                self_ty_name: type_spur,
                impl_def_id: landin_compiler::hir::DefId::new(0),
                entries: vec![VtableEntry {
                    method_name: interner.get_or_intern("clone"),
                    fn_name: format!("landin_{type_name}_clone"),
                }],
            },
        );
    }
    let s = build_trait_dispatch_emission_summary(&resolver, &interner);
    assert_eq!(s.vtable_count, 2);
    assert_eq!(s.trait_names, vec!["Clone".to_string()]); // deduplicated
    assert_eq!(s.type_names.len(), 2); // S + T
}

/// `type_names` deduplicated.
#[test]
fn test_build_trait_dispatch_emission_summary_type_names_dedup() {
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::new();
    // Same type (S) with two traits (Clone, Drop)
    for trait_name in ["Clone", "Drop"] {
        let trait_spur = interner.get_or_intern(trait_name);
        let type_spur = interner.get_or_intern("S");
        resolver.vtables.insert(
            (trait_spur, type_spur),
            Vtable {
                trait_name: trait_spur,
                self_ty_name: type_spur,
                impl_def_id: landin_compiler::hir::DefId::new(0),
                entries: vec![VtableEntry {
                    method_name: interner.get_or_intern("m"),
                    fn_name: format!("landin_S_{trait_name}_m"),
                }],
            },
        );
    }
    let s = build_trait_dispatch_emission_summary(&resolver, &interner);
    assert_eq!(s.vtable_count, 2);
    assert_eq!(s.type_names, vec!["S".to_string()]); // deduplicated
    assert_eq!(s.trait_names.len(), 2); // Clone + Drop
}

/// `total_method_slots` sums all vtable.entries.len().
#[test]
fn test_build_trait_dispatch_emission_summary_total_method_slots() {
    let mut interner = Rodeo::new();
    let mut resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_S_clone", "landin_S_clone_from"],
    );
    // Add Drop with 1 method
    let trait_spur = interner.get_or_intern("Drop");
    let type_spur = interner.get_or_intern("S");
    resolver.vtables.insert(
        (trait_spur, type_spur),
        Vtable {
            trait_name: trait_spur,
            self_ty_name: type_spur,
            impl_def_id: landin_compiler::hir::DefId::new(1),
            entries: vec![VtableEntry {
                method_name: interner.get_or_intern("drop"),
                fn_name: "landin_S_drop".to_string(),
            }],
        },
    );
    let s = build_trait_dispatch_emission_summary(&resolver, &interner);
    // Clone(2) + Drop(1) = 3
    assert_eq!(s.total_method_slots, 3);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

/// interner.try_resolve fails → "Trait"/"Type" defaults.
#[test]
fn test_build_trait_dispatch_emission_summary_unresolved_interner() {
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
    let s = build_trait_dispatch_emission_summary(&resolver, &fresh_interner);
    assert_eq!(s.vtable_count, 1);
    assert_eq!(s.trait_names, vec!["Trait".to_string()]); // default
    assert_eq!(s.type_names, vec!["Type".to_string()]); // default
}

/// Pure function — doesn't modify input TraitResolver.
#[test]
fn test_build_trait_dispatch_emission_summary_no_side_effects() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let vtables_count_before = resolver.vtables.len();

    let _s = build_trait_dispatch_emission_summary(&resolver, &interner);

    assert_eq!(resolver.vtables.len(), vtables_count_before);
}

// ---------------------------------------------------------------------------
// Real scenario + struct semantics
// ---------------------------------------------------------------------------

/// Simulate real TraitResolver with multiple (trait, type) pairs.
#[test]
fn test_build_trait_dispatch_emission_summary_real_scenario() {
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

    let s = build_trait_dispatch_emission_summary(&resolver, &interner);
    assert_eq!(s.vtable_count, 3);
    assert_eq!(s.dynptr_count, 3);
    assert_eq!(s.total_global_count, 6);
    assert_eq!(s.trait_names.len(), 3); // Clone + Drop + Display
    assert_eq!(s.type_names, vec!["S".to_string()]); // only S
                                                     // Clone(2) + Drop(1) + Display(1) = 4
    assert_eq!(s.total_method_slots, 4);
}

/// `CodegenTraitDispatchEmissionSummary` derives PartialEq/Eq.
#[test]
fn test_build_trait_dispatch_emission_summary_struct_eq() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let s1 = build_trait_dispatch_emission_summary(&resolver, &interner);
    let s2 = build_trait_dispatch_emission_summary(&resolver, &interner);

    assert_eq!(s1, s2);
}

/// Field access works.
#[test]
fn test_build_trait_dispatch_emission_summary_struct_field_access() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_S_drop"]);
    let s: CodegenTraitDispatchEmissionSummary =
        build_trait_dispatch_emission_summary(&resolver, &interner);
    assert_eq!(s.vtable_count, 1);
    assert_eq!(s.dynptr_count, 1);
    assert_eq!(s.total_global_count, 2);
    assert_eq!(s.trait_names, vec!["Drop".to_string()]);
    assert_eq!(s.type_names, vec!["S".to_string()]);
    assert_eq!(s.total_method_slots, 1);
}
