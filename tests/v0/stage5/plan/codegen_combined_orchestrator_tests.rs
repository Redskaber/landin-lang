//! Stage 5.51: Codegen vtable + dynptr combined emission orchestrator tests
//!
//! Tests `emit_vtables_and_dynptrs_from_resolver()` — combined orchestrator
//! that composes Stage 5.47's `emit_vtables_from_resolver()` + Stage 5.50's
//! `emit_dynptrs_from_resolver()`.
//!
//! **Critical invariant**: behavior must be identical to calling
//! `emit_vtables()` + `emit_dyn_trait_ptrs()` separately.
//! `test_emit_vtables_and_dynptrs_match_separate_calls` verifies this.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{
    emit_dyn_trait_ptrs, emit_vtables, emit_vtables_and_dynptrs_from_resolver, TextEmitter,
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
fn test_emit_vtables_and_dynptrs_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let mut emitter = TextEmitter::new();

    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(!output.contains(".vtable."));
    assert!(!output.contains(".dynptr."));
}

// ---------------------------------------------------------------------------
// Single vtable
// ---------------------------------------------------------------------------

/// Single vtable → vtable + dynptr global.
#[test]
fn test_emit_vtables_and_dynptrs_single() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let mut emitter = TextEmitter::new();

    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(output.contains("@.vtable.Foo.S"));
    assert!(output.contains("@.dynptr.Foo.S"));
}

// ---------------------------------------------------------------------------
// Multi vtable
// ---------------------------------------------------------------------------

/// Multiple vtables → multiple vtable + dynptr globals.
#[test]
fn test_emit_vtables_and_dynptrs_multi() {
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
                fn_name: interner.get_or_intern("landin_T_baz"),
            }],
        },
    );

    let mut emitter = TextEmitter::new();
    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(output.contains("@.vtable.Foo.S"));
    assert!(output.contains("@.vtable.Bar.T"));
    assert!(output.contains("@.dynptr.Foo.S"));
    assert!(output.contains("@.dynptr.Bar.T"));
}

// ---------------------------------------------------------------------------
// **Critical**: matches separate calls
// ---------------------------------------------------------------------------

/// `emit_vtables_and_dynptrs_from_resolver()` must produce identical output
/// to calling `emit_vtables()` + `emit_dyn_trait_ptrs()` separately.
#[test]
fn test_emit_vtables_and_dynptrs_match_separate_calls() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_S_clone", "landin_S_clone_from"],
    );

    // Call separate functions (existing path)
    let mut emitter1 = TextEmitter::new();
    emit_vtables(&resolver, &interner, &mut emitter1);
    emit_dyn_trait_ptrs(&resolver, &interner, &mut emitter1);
    let output1 = emitter1.output_with_globals();

    // Call combined orchestrator (new)
    let mut emitter2 = TextEmitter::new();
    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter2);
    let output2 = emitter2.output_with_globals();

    // Outputs must be identical
    assert_eq!(
        output1, output2,
        "combined orchestrator output differs from separate calls.\n\
         separate: {output1}\n\
         combined: {output2}"
    );
}

// ---------------------------------------------------------------------------
// No side effects on resolver
// ---------------------------------------------------------------------------

/// Pure with respect to TraitResolver — doesn't modify it.
#[test]
fn test_emit_vtables_and_dynptrs_no_side_effects_on_resolver() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let vtables_count_before = resolver.vtables.len();
    let mut emitter = TextEmitter::new();

    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter);

    assert_eq!(resolver.vtables.len(), vtables_count_before);
}

// ---------------------------------------------------------------------------
// Real scenario
// ---------------------------------------------------------------------------

/// Simulate real TraitResolver with multiple (trait, type) pairs.
#[test]
fn test_emit_vtables_and_dynptrs_real_scenario() {
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
    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    // 3 vtable global definitions + 3 dynptr global definitions
    // (count global *definitions*, not references inside dynptr initializers)
    let vtable_defs = output
        .lines()
        .filter(|line| {
            line.starts_with("@.vtable.") && line.contains("internal unnamed_addr constant")
        })
        .count();
    let dynptr_defs = output
        .lines()
        .filter(|line| {
            line.starts_with("@.dynptr.") && line.contains("internal unnamed_addr constant")
        })
        .count();
    assert_eq!(vtable_defs, 3);
    assert_eq!(dynptr_defs, 3);
    assert!(output.contains("@.vtable.Clone.S"));
    assert!(output.contains("@.dynptr.Clone.S"));
    assert!(output.contains("@.vtable.Drop.S"));
    assert!(output.contains("@.dynptr.Drop.S"));
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

/// interner.try_resolve fails → "Trait"/"Type" defaults.
#[test]
fn test_emit_vtables_and_dynptrs_unresolved_interner() {
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
                fn_name: interner_with_spur.get_or_intern("landin_S_bar"),
            }],
        },
    );

    // Use a *fresh* interner that doesn't know about these Spurs.
    let fresh_interner = Rodeo::new();
    let mut emitter = TextEmitter::new();

    emit_vtables_and_dynptrs_from_resolver(&resolver, &fresh_interner, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(output.contains("@.vtable.Trait.Type"));
    assert!(output.contains("@.dynptr.Trait.Type"));
}

/// Emitter receives the correct parameters for both vtable and dynptr.
#[test]
fn test_emit_vtables_and_dynptrs_emitter_called_correctly() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_S_drop"]);
    let mut emitter = TextEmitter::new();

    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    // vtable global
    assert!(output.contains("@.vtable.Drop.S = internal unnamed_addr constant"));
    assert!(output.contains("ptr @landin_S_drop"));
    // dynptr global
    assert!(output.contains("@.dynptr.Drop.S = internal unnamed_addr constant"));
    assert!(output.contains("ptr @.data.S"));
    assert!(output.contains("ptr @.vtable.Drop.S"));
}

/// Total global count == 2 × vtables.len() (one vtable + one dynptr per entry).
#[test]
fn test_emit_vtables_and_dynptrs_count_matches_vtables() {
    let mut interner = Rodeo::new();
    let mut resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
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
    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    // 3 vtable global definitions + 3 dynptr global definitions
    // Note: `@.vtable.` also appears in dynptr initializers (ptr @.vtable.X.Y),
    // so we count global *definitions* (lines starting with `@.vtable.` after
    // `constant`) — simpler: count `internal unnamed_addr constant` lines
    // that start with `@.vtable.` or `@.dynptr.`.
    let vtable_defs = output
        .lines()
        .filter(|line| {
            line.starts_with("@.vtable.") && line.contains("internal unnamed_addr constant")
        })
        .count();
    let dynptr_defs = output
        .lines()
        .filter(|line| {
            line.starts_with("@.dynptr.") && line.contains("internal unnamed_addr constant")
        })
        .count();
    assert_eq!(vtable_defs, 3);
    assert_eq!(dynptr_defs, 3);
}

// ---------------------------------------------------------------------------
// Composition + determinism
// ---------------------------------------------------------------------------

/// Combined orchestrator composes both vtable + dynptr emission.
#[test]
fn test_emit_vtables_and_dynptrs_composes_both() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let mut emitter = TextEmitter::new();

    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    // Both vtable and dynptr globals present
    assert!(output.contains("@.vtable.Foo.S"));
    assert!(output.contains("@.dynptr.Foo.S"));
}

/// Repeated calls produce identical output (deterministic count).
#[test]
fn test_emit_vtables_and_dynptrs_deterministic_count() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let mut emitter1 = TextEmitter::new();
    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter1);
    let output1 = emitter1.output_with_globals();

    let mut emitter2 = TextEmitter::new();
    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter2);
    let output2 = emitter2.output_with_globals();

    let vtable_count1 = output1.matches("@.vtable.").count();
    let vtable_count2 = output2.matches("@.vtable.").count();
    let dynptr_count1 = output1.matches("@.dynptr.").count();
    let dynptr_count2 = output2.matches("@.dynptr.").count();
    assert_eq!(vtable_count1, vtable_count2);
    assert_eq!(dynptr_count1, dynptr_count2);
}

/// vtable globals appear before dynptr globals (order from composition).
#[test]
fn test_emit_vtables_and_dynptrs_order() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let mut emitter = TextEmitter::new();

    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter);

    let output = emitter.output_with_globals();
    let vtable_pos = output.find("@.vtable.Foo.S");
    let dynptr_pos = output.find("@.dynptr.Foo.S");
    assert!(vtable_pos.is_some());
    assert!(dynptr_pos.is_some());
    // vtable should appear before dynptr (since emit_vtables is called first)
    assert!(
        vtable_pos.unwrap() < dynptr_pos.unwrap(),
        "vtable should appear before dynptr in output"
    );
}
