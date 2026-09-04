//! Stage 5.72: build_dyn_trait_mir_summary_from_resolver tests
//!
//! Tests the convenience entry point that combines Stage 5.62 + 5.68 + 5.71.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::build_dyn_trait_mir_summary_from_resolver;
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

/// Empty TraitResolver → all-zero summary.
#[test]
fn test_summary_from_resolver_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let s = build_dyn_trait_mir_summary_from_resolver(&resolver, &interner);
    assert_eq!(s.fat_ptr_count, 0);
    assert_eq!(s.method_call_count, 0);
    assert_eq!(s.total_slots, 0);
}

/// Single vtable (Clone) → 1 fat ptr + 2 method calls.
#[test]
fn test_summary_from_resolver_clone() {
    let mut interner = Rodeo::new();
    let resolver =
        make_resolver_with_vtable(&mut interner, "Clone", "S", &["landin_Clone_S_clone"]);
    let s = build_dyn_trait_mir_summary_from_resolver(&resolver, &interner);
    assert_eq!(s.fat_ptr_count, 1);
    assert_eq!(s.method_call_count, 2); // clone + clone_from
    assert_eq!(s.total_slots, 2);
    assert_eq!(s.trait_names, vec!["Clone".to_string()]);
    assert_eq!(s.type_names, vec!["S".to_string()]);
}

/// Drop → 1 fat ptr + 1 method call.
#[test]
fn test_summary_from_resolver_drop() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_Drop_S_drop"]);
    let s = build_dyn_trait_mir_summary_from_resolver(&resolver, &interner);
    assert_eq!(s.fat_ptr_count, 1);
    assert_eq!(s.method_call_count, 1);
    assert_eq!(s.total_slots, 1);
}

/// No side effects on resolver.
#[test]
fn test_summary_from_resolver_no_side_effects() {
    let mut interner = Rodeo::new();
    let resolver =
        make_resolver_with_vtable(&mut interner, "Clone", "S", &["landin_Clone_S_clone"]);
    let count_before = resolver.vtables.len();
    let _ = build_dyn_trait_mir_summary_from_resolver(&resolver, &interner);
    assert_eq!(resolver.vtables.len(), count_before);
}

/// Real scenario: S impls Clone + Drop + Display.
#[test]
fn test_summary_from_resolver_real_scenario() {
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::new();
    for (trait_name, methods) in [
        (
            "Clone",
            vec!["landin_Clone_S_clone", "landin_Clone_S_clone_from"],
        ),
        ("Drop", vec!["landin_Drop_S_drop"]),
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
    let s = build_dyn_trait_mir_summary_from_resolver(&resolver, &interner);
    assert_eq!(s.fat_ptr_count, 3);
    assert_eq!(s.method_call_count, 4); // Clone(2) + Drop(1) + Display(1)
    assert_eq!(s.total_slots, 2); // max slot is 1 (clone_from)
    assert_eq!(s.trait_names.len(), 3);
    assert_eq!(s.type_names.len(), 1); // all "S"
}

/// Deterministic — repeated calls identical.
#[test]
fn test_summary_from_resolver_deterministic() {
    let mut interner = Rodeo::new();
    let resolver =
        make_resolver_with_vtable(&mut interner, "Clone", "S", &["landin_Clone_S_clone"]);
    let s1 = build_dyn_trait_mir_summary_from_resolver(&resolver, &interner);
    let s2 = build_dyn_trait_mir_summary_from_resolver(&resolver, &interner);
    assert_eq!(s1, s2);
}

/// Marker trait (Copy) → 0 method calls, 0 slots.
#[test]
fn test_summary_from_resolver_marker() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Copy", "S", &[]);
    let s = build_dyn_trait_mir_summary_from_resolver(&resolver, &interner);
    assert_eq!(s.fat_ptr_count, 1);
    assert_eq!(s.method_call_count, 0);
    assert_eq!(s.total_slots, 0);
}

/// PartialEq → 2 method calls, 2 slots.
#[test]
fn test_summary_from_resolver_partial_eq() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "PartialEq", "S", &["landin_S_eq"]);
    let s = build_dyn_trait_mir_summary_from_resolver(&resolver, &interner);
    assert_eq!(s.method_call_count, 2); // eq + ne
    assert_eq!(s.total_slots, 2);
}
