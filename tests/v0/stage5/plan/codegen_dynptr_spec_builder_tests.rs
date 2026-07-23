//! Stage 5.49: Codegen dynptr spec builder tests
//!
//! Tests `StdlibDynptrGlobalSpec` struct + `build_dynptr_global_specs()` —
//! pure free function that extracts the spec-construction logic from
//! `emit_dyn_trait_ptrs()` into a standalone function.
//!
//! **Critical invariant**: the output must match what `emit_dyn_trait_ptrs()`
//! currently constructs inline. `test_build_dynptr_global_specs_match_emit_dyn_trait_ptrs`
//! verifies this by manually inlining the same logic and asserting equality.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{build_dynptr_global_specs, StdlibDynptrGlobalSpec};
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

/// Empty TraitResolver → empty Vec.
#[test]
fn test_build_dynptr_global_specs_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let specs = build_dynptr_global_specs(&resolver, &interner);
    assert!(specs.is_empty());
}

// ---------------------------------------------------------------------------
// Single vtable
// ---------------------------------------------------------------------------

/// Single vtable → 1 spec with correct global_name + data_symbol + vtable_symbol.
#[test]
fn test_build_dynptr_global_specs_single() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let specs = build_dynptr_global_specs(&resolver, &interner);
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].global_name, ".dynptr.Foo.S");
    assert_eq!(specs[0].data_symbol, ".data.S");
    assert_eq!(specs[0].vtable_symbol, ".vtable.Foo.S");
}

// ---------------------------------------------------------------------------
// Multi vtable
// ---------------------------------------------------------------------------

/// Multiple vtables → multiple specs.
#[test]
fn test_build_dynptr_global_specs_multi() {
    let mut interner = Rodeo::new();
    let mut resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    // Add second vtable for (Bar, T)
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

    let specs = build_dynptr_global_specs(&resolver, &interner);
    assert_eq!(specs.len(), 2);
    // HashMap order non-deterministic — use set comparison
    let global_names: Vec<&str> = specs.iter().map(|s| s.global_name.as_str()).collect();
    assert!(global_names.contains(&".dynptr.Foo.S"));
    assert!(global_names.contains(&".dynptr.Bar.T"));
}

// ---------------------------------------------------------------------------
// Format components
// ---------------------------------------------------------------------------

/// global_name format: `.dynptr.<trait>.<type>`.
#[test]
fn test_build_dynptr_global_specs_global_name_format() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Display", "Vec", &["landin_Vec_fmt"]);

    let specs = build_dynptr_global_specs(&resolver, &interner);
    assert_eq!(specs[0].global_name, ".dynptr.Display.Vec");
}

/// data_symbol format: `.data.<type>`.
#[test]
fn test_build_dynptr_global_specs_data_symbol() {
    let mut interner = Rodeo::new();
    let resolver =
        make_resolver_with_vtable(&mut interner, "Foo", "MyType", &["landin_MyType_bar"]);

    let specs = build_dynptr_global_specs(&resolver, &interner);
    assert_eq!(specs[0].data_symbol, ".data.MyType");
}

/// vtable_symbol format: `.vtable.<trait>.<type>`.
#[test]
fn test_build_dynptr_global_specs_vtable_symbol() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Clone", "S", &["landin_S_clone"]);

    let specs = build_dynptr_global_specs(&resolver, &interner);
    assert_eq!(specs[0].vtable_symbol, ".vtable.Clone.S");
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

/// interner.try_resolve fails → "Trait"/"Type" defaults.
#[test]
fn test_build_dynptr_global_specs_unresolved_interner() {
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
    let specs = build_dynptr_global_specs(&resolver, &fresh_interner);
    assert_eq!(specs.len(), 1);
    // try_resolve fails → defaults "Trait"/"Type"
    assert_eq!(specs[0].global_name, ".dynptr.Trait.Type");
    assert_eq!(specs[0].data_symbol, ".data.Type");
    assert_eq!(specs[0].vtable_symbol, ".vtable.Trait.Type");
}

/// Pure function — doesn't modify input TraitResolver or interner.
#[test]
fn test_build_dynptr_global_specs_no_side_effects() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let vtables_count_before = resolver.vtables.len();

    let _specs = build_dynptr_global_specs(&resolver, &interner);

    assert_eq!(resolver.vtables.len(), vtables_count_before);
}

/// Deterministic — repeated calls return same spec content.
#[test]
fn test_build_dynptr_global_specs_deterministic() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let specs1 = build_dynptr_global_specs(&resolver, &interner);
    let specs2 = build_dynptr_global_specs(&resolver, &interner);

    assert_eq!(specs1.len(), specs2.len());
    let s1 = specs1
        .iter()
        .find(|s| s.global_name == ".dynptr.Foo.S")
        .expect("spec should exist");
    let s2 = specs2
        .iter()
        .find(|s| s.global_name == ".dynptr.Foo.S")
        .expect("spec should exist");
    assert_eq!(s1, s2);
}

// ---------------------------------------------------------------------------
// **Critical**: matches emit_dyn_trait_ptrs() inline construction
// ---------------------------------------------------------------------------

/// The output of `build_dynptr_global_specs()` must match what
/// `emit_dyn_trait_ptrs()` currently constructs inline.
#[test]
fn test_build_dynptr_global_specs_match_emit_dyn_trait_ptrs() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_S_clone", "landin_S_clone_from"],
    );

    // Manually inline the emit_dyn_trait_ptrs() construction logic
    let mut expected_specs: Vec<StdlibDynptrGlobalSpec> = Vec::new();
    for (trait_name, self_ty_name) in resolver.vtables.keys() {
        let trait_str = interner.try_resolve(trait_name).unwrap_or("Trait");
        let type_str = interner.try_resolve(self_ty_name).unwrap_or("Type");
        expected_specs.push(StdlibDynptrGlobalSpec {
            global_name: format!(".dynptr.{trait_str}.{type_str}"),
            data_symbol: format!(".data.{type_str}"),
            vtable_symbol: format!(".vtable.{trait_str}.{type_str}"),
        });
    }

    let actual_specs = build_dynptr_global_specs(&resolver, &interner);

    // Compare as sets (HashMap order may differ)
    assert_eq!(actual_specs.len(), expected_specs.len());
    for expected in &expected_specs {
        assert!(
            actual_specs.iter().any(|actual| actual == expected),
            "expected spec {expected:?} not found in actual {actual_specs:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Integration: build + emit_dynptr_global_text
// ---------------------------------------------------------------------------

/// `build_dynptr_global_specs()` + `emit_dynptr_global_text()` per spec →
/// complete LLVM IR text. This is the Stage 5.50 refactored flow.
#[test]
fn test_build_dynptr_global_specs_then_emit() {
    use landin_compiler::codegen::emit_dynptr_global_text;
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_S_drop"]);

    let specs = build_dynptr_global_specs(&resolver, &interner);
    assert_eq!(specs.len(), 1);

    let ir = emit_dynptr_global_text(
        &specs[0].global_name,
        &specs[0].data_symbol,
        &specs[0].vtable_symbol,
    );
    assert_eq!(
        ir,
        "@.dynptr.Drop.S = private unnamed_addr constant \
         { ptr, ptr } { ptr @.data.S, ptr @.vtable.Drop.S }"
    );
}

// ---------------------------------------------------------------------------
// Real scenario simulation
// ---------------------------------------------------------------------------

/// Simulate a real TraitResolver with multiple (trait, type) pairs.
#[test]
fn test_build_dynptr_global_specs_real_scenario() {
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

    let specs = build_dynptr_global_specs(&resolver, &interner);
    assert_eq!(specs.len(), 3);

    // All global names should start with .dynptr. and end with .S
    for spec in &specs {
        assert!(spec.global_name.starts_with(".dynptr."));
        assert!(spec.global_name.ends_with(".S"));
        // All share the same data symbol (.data.S) since same type
        assert_eq!(spec.data_symbol, ".data.S");
    }

    // Verify each spec's vtable_symbol
    let clone_spec = specs
        .iter()
        .find(|s| s.global_name == ".dynptr.Clone.S")
        .expect("Clone spec should exist");
    assert_eq!(clone_spec.vtable_symbol, ".vtable.Clone.S");
}
