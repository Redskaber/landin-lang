//! Stage 5.62: build_dyn_trait_fat_ptrs_from_resolver tests
//!
//! Tests `build_dyn_trait_fat_ptrs_from_resolver()` — bridge function
//! that constructs `Vec<DynTraitFatPtr>` from `TraitResolver.vtables`.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::build_dyn_trait_fat_ptrs_from_resolver;
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
            fn_name: sym.to_string(),
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

/// Empty TraitResolver → empty Vec.
#[test]
fn test_build_dyn_trait_fat_ptrs_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert!(fat_ptrs.is_empty());
}

/// Single vtable → 1 fat ptr.
#[test]
fn test_build_dyn_trait_fat_ptrs_single() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert_eq!(fat_ptrs.len(), 1);
    assert_eq!(fat_ptrs[0].trait_name, "Foo");
    assert_eq!(fat_ptrs[0].type_name, "S");
    assert_eq!(fat_ptrs[0].data_symbol, ".data.S");
    assert_eq!(fat_ptrs[0].vtable_symbol, ".vtable.Foo.S");
    assert_eq!(fat_ptrs[0].dynptr_symbol, ".dynptr.Foo.S");
}

/// Multiple vtables → multiple fat ptrs.
#[test]
fn test_build_dyn_trait_fat_ptrs_multi() {
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
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert_eq!(fat_ptrs.len(), 2);
    let names: Vec<&str> = fat_ptrs.iter().map(|fp| fp.trait_name.as_str()).collect();
    assert!(names.contains(&"Foo"));
    assert!(names.contains(&"Bar"));
}

/// Unresolved interner → "Trait"/"Type" defaults.
#[test]
fn test_build_dyn_trait_fat_ptrs_unresolved_interner() {
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
    let fresh_interner = Rodeo::new();
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &fresh_interner);
    assert_eq!(fat_ptrs.len(), 1);
    assert_eq!(fat_ptrs[0].trait_name, "Trait");
    assert_eq!(fat_ptrs[0].type_name, "Type");
}

/// No side effects on resolver.
#[test]
fn test_build_dyn_trait_fat_ptrs_no_side_effects() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let count_before = resolver.vtables.len();
    let _ = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert_eq!(resolver.vtables.len(), count_before);
}

/// Marker trait detection works after building from resolver.
#[test]
fn test_build_dyn_trait_fat_ptrs_marker_detection() {
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::new();
    let trait_spur = interner.get_or_intern("Copy");
    let type_spur = interner.get_or_intern("S");
    resolver.vtables.insert(
        (trait_spur, type_spur),
        Vtable {
            trait_name: trait_spur,
            self_ty_name: type_spur,
            impl_def_id: landin_compiler::hir::DefId::new(0),
            entries: vec![],
        },
    );
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert_eq!(fat_ptrs.len(), 1);
    assert!(fat_ptrs[0].is_marker());
}

/// Real scenario: S impls Clone + Drop + Display.
#[test]
fn test_build_dyn_trait_fat_ptrs_real_scenario() {
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
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert_eq!(fat_ptrs.len(), 3);
    // All share the same data_symbol (same type S)
    for fp in &fat_ptrs {
        assert_eq!(fp.data_symbol, ".data.S");
        assert!(!fp.is_marker());
    }
    // Each has unique vtable_symbol
    let vtable_syms: Vec<&str> = fat_ptrs
        .iter()
        .map(|fp| fp.vtable_symbol.as_str())
        .collect();
    assert!(vtable_syms.contains(&".vtable.Clone.S"));
    assert!(vtable_syms.contains(&".vtable.Drop.S"));
    assert!(vtable_syms.contains(&".vtable.Display.S"));
}

/// Deterministic — repeated calls return same count.
#[test]
fn test_build_dyn_trait_fat_ptrs_deterministic() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let fps1 = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    let fps2 = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert_eq!(fps1.len(), fps2.len());
}
