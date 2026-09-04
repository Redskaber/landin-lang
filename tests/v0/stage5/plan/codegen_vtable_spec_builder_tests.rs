//! Stage 5.46: Codegen vtable spec builder tests
//!
//! Tests `build_vtable_global_specs()` — pure free function that extracts
//! the spec-construction logic from `emit_vtables()` into a standalone
//! function.
//!
//! **Critical invariant**: the output must match what `emit_vtables()`
//! currently constructs inline. `test_build_vtable_global_specs_match_emit_vtables_inline`
//! verifies this by manually inlining the same logic and asserting equality.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{
    build_vtable_global_specs, emit_vtable_globals_batch, StdlibVtableGlobalSpec,
};
use landin_compiler::traits::{TraitResolver, Vtable, VtableEntry};
use lasso::Rodeo;

// ---------------------------------------------------------------------------
// Helper: construct a TraitResolver with vtables for testing
// ---------------------------------------------------------------------------

/// Build a TraitResolver with one vtable for testing.
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
fn test_build_vtable_global_specs_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let specs = build_vtable_global_specs(&resolver, &interner);
    assert!(specs.is_empty());
}

// ---------------------------------------------------------------------------
// Single vtable
// ---------------------------------------------------------------------------

/// Single vtable → 1 spec with correct global_name + method_symbols.
#[test]
fn test_build_vtable_global_specs_single() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_Foo_S_bar"]);

    let specs = build_vtable_global_specs(&resolver, &interner);
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].global_name, ".vtable.Foo.S");
    assert_eq!(
        specs[0].method_symbols,
        vec!["landin_Foo_S_bar".to_string()]
    );
}

// ---------------------------------------------------------------------------
// Multi vtable
// ---------------------------------------------------------------------------

/// Multiple vtables → multiple specs.
#[test]
fn test_build_vtable_global_specs_multi() {
    let mut interner = Rodeo::new();
    let mut resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_Foo_S_bar"]);
    // Add second vtable for (Bar, T)
    let trait_spur = interner.get_or_intern("Bar");
    let type_spur = interner.get_or_intern("T");
    let vtable = Vtable {
        trait_name: trait_spur,
        self_ty_name: type_spur,
        impl_def_id: landin_compiler::hir::DefId::new(1),
        entries: vec![VtableEntry {
            method_name: interner.get_or_intern("baz"),
            fn_name: interner.get_or_intern("landin_T_baz"),
        }],
    };
    resolver.vtables.insert((trait_spur, type_spur), vtable);

    let specs = build_vtable_global_specs(&resolver, &interner);
    assert_eq!(specs.len(), 2);
    // Note: HashMap iteration order is not deterministic, so we check
    // that both expected specs are present (set comparison).
    let global_names: Vec<&str> = specs.iter().map(|s| s.global_name.as_str()).collect();
    assert!(global_names.contains(&".vtable.Foo.S"));
    assert!(global_names.contains(&".vtable.Bar.T"));
}

// ---------------------------------------------------------------------------
// Format components
// ---------------------------------------------------------------------------

/// global_name format: `.vtable.<trait>.<type>`.
#[test]
fn test_build_vtable_global_specs_global_name_format() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Display", "Vec", &["landin_Vec_fmt"]);

    let specs = build_vtable_global_specs(&resolver, &interner);
    assert_eq!(specs[0].global_name, ".vtable.Display.Vec");
}

/// method_symbols extracted from VtableEntry.fn_name.
#[test]
fn test_build_vtable_global_specs_method_symbols() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_Clone_S_clone", "landin_Clone_S_clone_from"],
    );

    let specs = build_vtable_global_specs(&resolver, &interner);
    assert_eq!(specs[0].method_symbols.len(), 2);
    assert_eq!(specs[0].method_symbols[0], "landin_Clone_S_clone");
    assert_eq!(specs[0].method_symbols[1], "landin_Clone_S_clone_from");
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

/// interner.try_resolve fails → "Trait"/"Type" defaults.
///
/// We construct a vtable with a real Spur (interned), then use a *different*
/// fresh Rodeo that doesn't know about that Spur — simulating "unresolved".
#[test]
fn test_build_vtable_global_specs_unresolved_interner() {
    let mut interner_with_spur = Rodeo::new();
    let trait_spur = interner_with_spur.get_or_intern("Foo");
    let type_spur = interner_with_spur.get_or_intern("S");

    let mut resolver = TraitResolver::new();
    let vtable = Vtable {
        trait_name: trait_spur,
        self_ty_name: type_spur,
        impl_def_id: landin_compiler::hir::DefId::new(0),
        entries: vec![],
    };
    resolver.vtables.insert((trait_spur, type_spur), vtable);

    // Use a *fresh* interner that doesn't know about these Spurs.
    let fresh_interner = Rodeo::new();
    let specs = build_vtable_global_specs(&resolver, &fresh_interner);
    assert_eq!(specs.len(), 1);
    // try_resolve fails on fresh interner → defaults "Trait"/"Type"
    assert_eq!(specs[0].global_name, ".vtable.Trait.Type");
}

/// Pure function — doesn't modify input TraitResolver or interner.
#[test]
fn test_build_vtable_global_specs_no_side_effects() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_Foo_S_bar"]);

    // Snapshot the vtables count before
    let vtables_count_before = resolver.vtables.len();

    let _specs = build_vtable_global_specs(&resolver, &interner);

    // No mutation
    assert_eq!(resolver.vtables.len(), vtables_count_before);
}

/// Deterministic within a single call (specs correspond 1:1 to vtables).
#[test]
fn test_build_vtable_global_specs_deterministic() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Foo",
        "S",
        &["landin_Foo_S_bar", "landin_S_baz"],
    );

    let specs1 = build_vtable_global_specs(&resolver, &interner);
    let specs2 = build_vtable_global_specs(&resolver, &interner);

    // Same length (HashMap order may differ, but count is deterministic)
    assert_eq!(specs1.len(), specs2.len());
    // Find the spec in both
    let s1 = specs1
        .iter()
        .find(|s| s.global_name == ".vtable.Foo.S")
        .expect("spec should exist");
    let s2 = specs2
        .iter()
        .find(|s| s.global_name == ".vtable.Foo.S")
        .expect("spec should exist");
    assert_eq!(s1, s2);
}

// ---------------------------------------------------------------------------
// **Critical**: matches emit_vtables() inline construction
// ---------------------------------------------------------------------------

/// The output of `build_vtable_global_specs()` must match what `emit_vtables()`
/// currently constructs inline. We manually inline the same logic here and
/// assert equality.
#[test]
fn test_build_vtable_global_specs_match_emit_vtables_inline() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_Clone_S_clone", "landin_Clone_S_clone_from"],
    );

    // Manually inline the emit_vtables() construction logic
    let mut expected_specs: Vec<StdlibVtableGlobalSpec> = Vec::new();
    for ((trait_name, self_ty_name), vtable) in &resolver.vtables {
        let trait_str = interner.try_resolve(trait_name).unwrap_or("Trait");
        let type_str = interner.try_resolve(self_ty_name).unwrap_or("Type");
        let global_name = format!(".vtable.{trait_str}.{type_str}");
        let method_symbols: Vec<String> = vtable
            .entries
            .iter()
            .map(|e| {
                interner
                    .try_resolve(&e.fn_name)
                    .map(String::from)
                    .unwrap_or_else(|| "fn".to_string())
            })
            .collect();
        expected_specs.push(StdlibVtableGlobalSpec {
            global_name,
            method_symbols,
        });
    }

    let actual_specs = build_vtable_global_specs(&resolver, &interner);

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
// Integration: build + batch → complete IR text
// ---------------------------------------------------------------------------

/// `build_vtable_global_specs()` + `emit_vtable_globals_batch()` → complete
/// LLVM IR text for all vtables. This is the Stage 5.47 refactored
/// `emit_vtables()` flow.
#[test]
fn test_build_vtable_global_specs_then_batch_emit() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_Drop_S_drop"]);

    let specs = build_vtable_global_specs(&resolver, &interner);
    let ir_lines = emit_vtable_globals_batch(&specs);

    assert_eq!(ir_lines.len(), 1);
    assert_eq!(
        ir_lines[0],
        "@.vtable.Drop.S = internal unnamed_addr constant [1 x ptr] [ptr @landin_Drop_S_drop]"
    );
}

// ---------------------------------------------------------------------------
// Empty vtable entries
// ---------------------------------------------------------------------------

/// Vtable with empty entries → spec with empty method_symbols.
/// (This would emit `zeroinitializer` via `emit_vtable_global_text`.)
#[test]
fn test_build_vtable_global_specs_empty_vtable_entries() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &[]);

    let specs = build_vtable_global_specs(&resolver, &interner);
    assert_eq!(specs.len(), 1);
    assert!(specs[0].method_symbols.is_empty());
    // Verify the spec produces zeroinitializer when fed to batch emit
    let ir = emit_vtable_globals_batch(&specs);
    assert!(ir[0].contains("zeroinitializer"));
}

// ---------------------------------------------------------------------------
// Real scenario simulation
// ---------------------------------------------------------------------------

/// Simulate a real TraitResolver with multiple (trait, type) pairs.
#[test]
fn test_build_vtable_global_specs_real_scenario() {
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::new();

    // S impls Clone + Drop + Display
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

    let specs = build_vtable_global_specs(&resolver, &interner);
    assert_eq!(specs.len(), 3);

    // All global names should start with .vtable.
    for spec in &specs {
        assert!(spec.global_name.starts_with(".vtable."));
        assert!(spec.global_name.ends_with(".S"));
    }

    // Verify each spec's method_symbols
    let clone_spec = specs
        .iter()
        .find(|s| s.global_name == ".vtable.Clone.S")
        .expect("Clone spec should exist");
    assert_eq!(clone_spec.method_symbols.len(), 2);

    let drop_spec = specs
        .iter()
        .find(|s| s.global_name == ".vtable.Drop.S")
        .expect("Drop spec should exist");
    assert_eq!(drop_spec.method_symbols.len(), 1);
}
