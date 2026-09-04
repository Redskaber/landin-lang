//! Stage 5.47: Codegen vtable emission orchestrator tests
//!
//! Tests `emit_vtables_from_resolver()` — orchestrator that composes
//! Stage 5.46's `build_vtable_global_specs()` + per-spec
//! `Emitter::emit_vtable_global()` calls.
//!
//! **Critical invariant**: behavior must be identical to `emit_vtables()`
//! (Stage 5.6) current inline loop. `test_emit_vtables_from_resolver_match_emit_vtables`
//! verifies this by calling both on the same TraitResolver + interner +
//! TextEmitter pair and asserting the outputs are identical.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{emit_vtables, emit_vtables_from_resolver, TextEmitter};
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

/// Empty TraitResolver → no emitter calls, empty output.
#[test]
fn test_emit_vtables_from_resolver_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let mut emitter = TextEmitter::new();

    emit_vtables_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    // No vtable globals should appear (only the "; --- Module-level ... ---"
    // header if any globals exist, but with empty resolver there are none).
    assert!(!output.contains(".vtable."));
}

// ---------------------------------------------------------------------------
// Single vtable
// ---------------------------------------------------------------------------

/// Single vtable → 1 emitter call, output contains the vtable global.
#[test]
fn test_emit_vtables_from_resolver_single() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_Foo_S_bar"]);
    let mut emitter = TextEmitter::new();

    emit_vtables_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(output.contains("@.vtable.Foo.S"));
    assert!(output.contains("ptr @landin_Foo_S_bar"));
}

// ---------------------------------------------------------------------------
// Multi vtable
// ---------------------------------------------------------------------------

/// Multiple vtables → multiple emitter calls, output contains all.
#[test]
fn test_emit_vtables_from_resolver_multi() {
    let mut interner = Rodeo::new();
    let mut resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_Foo_S_bar"]);

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
                fn_name: interner.get_or_intern("landin_T_baz"),
            }],
        },
    );

    let mut emitter = TextEmitter::new();
    emit_vtables_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(output.contains("@.vtable.Foo.S"));
    assert!(output.contains("@.vtable.Bar.T"));
}

// ---------------------------------------------------------------------------
// **Critical**: behavior matches emit_vtables()
// ---------------------------------------------------------------------------

/// `emit_vtables_from_resolver()` must produce identical output to
/// `emit_vtables()` (Stage 5.6) on the same inputs.
#[test]
fn test_emit_vtables_from_resolver_match_emit_vtables() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_Clone_S_clone", "landin_Clone_S_clone_from"],
    );

    // Call emit_vtables() (existing path)
    let mut emitter1 = TextEmitter::new();
    emit_vtables(&resolver, &interner, &mut emitter1);
    let output1 = emitter1.output_with_globals();

    // Call emit_vtables_from_resolver() (new orchestrator)
    let mut emitter2 = TextEmitter::new();
    emit_vtables_from_resolver(&resolver, &interner, &mut emitter2);
    let output2 = emitter2.output_with_globals();

    // Outputs must be identical
    assert_eq!(
        output1, output2,
        "emit_vtables_from_resolver output differs from emit_vtables.\n\
         emit_vtables: {output1}\n\
         orchestrator: {output2}"
    );
}

/// Same cross-check with multiple vtables.
#[test]
fn test_emit_vtables_from_resolver_match_emit_vtables_multi() {
    let mut interner = Rodeo::new();
    let mut resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_Foo_S_bar"]);
    // Add second vtable
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
                fn_name: interner.get_or_intern("landin_Drop_S_drop"),
            }],
        },
    );

    let mut emitter1 = TextEmitter::new();
    emit_vtables(&resolver, &interner, &mut emitter1);
    let output1 = emitter1.output_with_globals();

    let mut emitter2 = TextEmitter::new();
    emit_vtables_from_resolver(&resolver, &interner, &mut emitter2);
    let output2 = emitter2.output_with_globals();

    assert_eq!(output1, output2);
}

// ---------------------------------------------------------------------------
// No side effects on resolver
// ---------------------------------------------------------------------------

/// Pure with respect to TraitResolver — doesn't modify it.
#[test]
fn test_emit_vtables_from_resolver_no_side_effects_on_resolver() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_Foo_S_bar"]);
    let vtables_count_before = resolver.vtables.len();
    let mut emitter = TextEmitter::new();

    emit_vtables_from_resolver(&resolver, &interner, &mut emitter);

    assert_eq!(resolver.vtables.len(), vtables_count_before);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

/// Vtable with empty entries → still calls emitter (emits zeroinitializer).
#[test]
fn test_emit_vtables_from_resolver_empty_entries() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &[]);
    let mut emitter = TextEmitter::new();

    emit_vtables_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(output.contains("@.vtable.Foo.S"));
    assert!(output.contains("zeroinitializer"));
}

/// interner.try_resolve fails → "Trait"/"Type" defaults.
#[test]
fn test_emit_vtables_from_resolver_unresolved_interner() {
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
            entries: vec![VtableEntry {
                method_name: trait_spur,
                fn_name: interner_with_spur.get_or_intern("landin_Foo_S_bar"),
            }],
        },
    );

    // Use a *fresh* interner that doesn't know about these Spurs.
    let fresh_interner = Rodeo::new();
    let mut emitter = TextEmitter::new();

    emit_vtables_from_resolver(&resolver, &fresh_interner, &mut emitter);

    let output = emitter.output_with_globals();
    // try_resolve fails → defaults "Trait"/"Type"
    assert!(output.contains("@.vtable.Trait.Type"));
}

// ---------------------------------------------------------------------------
// Emitter called correctly
// ---------------------------------------------------------------------------

/// Emitter receives the correct global_name and method_symbols per spec.
#[test]
fn test_emit_vtables_from_resolver_emitter_called_correctly() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_Clone_S_clone", "landin_Clone_S_clone_from"],
    );
    let mut emitter = TextEmitter::new();

    emit_vtables_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    // Verify the full LLVM IR line is correct
    assert!(output.contains(
        "@.vtable.Clone.S = internal unnamed_addr constant \
         [2 x ptr] [ptr @landin_Clone_S_clone, ptr @landin_Clone_S_clone_from]"
    ));
}

/// Number of emitter calls == number of vtables in resolver.
#[test]
fn test_emit_vtables_from_resolver_count_matches_vtables() {
    let mut interner = Rodeo::new();
    let mut resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_Foo_S_bar"]);
    // Add 2 more vtables
    for (trait_name, type_name) in [("Bar", "T"), ("Baz", "U")] {
        let trait_spur = interner.get_or_intern(trait_name);
        let type_spur = interner.get_or_intern(type_name);
        resolver.vtables.insert(
            (trait_spur, type_spur),
            Vtable {
                trait_name: trait_spur,
                self_ty_name: type_spur,
                impl_def_id: landin_compiler::hir::DefId::new(0),
                entries: vec![],
            },
        );
    }
    assert_eq!(resolver.vtables.len(), 3);

    let mut emitter = TextEmitter::new();
    emit_vtables_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    // Count occurrences of "@.vtable." — should be 3
    let count = output.matches("@.vtable.").count();
    assert_eq!(count, 3);
}

// ---------------------------------------------------------------------------
// Composition + determinism
// ---------------------------------------------------------------------------

/// Orchestrator composes build_vtable_global_specs + emit_vtable_global.
/// Verify by checking output contains all expected vtable lines.
#[test]
fn test_emit_vtables_from_resolver_composes_build_and_emit() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_Drop_S_drop"]);
    let mut emitter = TextEmitter::new();

    emit_vtables_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    // The orchestrator should have:
    // 1. Called build_vtable_global_specs (which constructs ".vtable.Drop.S")
    // 2. Called emitter.emit_vtable_global with that spec
    // → output contains the full IR line
    assert!(output.contains("@.vtable.Drop.S = internal unnamed_addr constant"));
    assert!(output.contains("ptr @landin_Drop_S_drop"));
}

/// Repeated calls produce identical output (deterministic).
#[test]
fn test_emit_vtables_from_resolver_deterministic_count() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_Foo_S_bar"]);

    let mut emitter1 = TextEmitter::new();
    emit_vtables_from_resolver(&resolver, &interner, &mut emitter1);
    let output1 = emitter1.output_with_globals();

    let mut emitter2 = TextEmitter::new();
    emit_vtables_from_resolver(&resolver, &interner, &mut emitter2);
    let output2 = emitter2.output_with_globals();

    // Same vtable count (HashMap order may differ, but count is deterministic)
    let count1 = output1.matches("@.vtable.").count();
    let count2 = output2.matches("@.vtable.").count();
    assert_eq!(count1, count2);
}

// ---------------------------------------------------------------------------
// Real scenario
// ---------------------------------------------------------------------------

/// Simulate real TraitResolver with multiple (trait, type) pairs.
#[test]
fn test_emit_vtables_from_resolver_real_scenario() {
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

    let mut emitter = TextEmitter::new();
    emit_vtables_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    // All 3 vtables should be present
    assert_eq!(output.matches("@.vtable.").count(), 3);
    assert!(output.contains("@.vtable.Clone.S"));
    assert!(output.contains("@.vtable.Drop.S"));
    assert!(output.contains("@.vtable.Display.S"));
}
