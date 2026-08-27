//! Stage 5.56: Codegen trait-dispatch emission text batch from resolver tests
//!
//! Tests `emit_trait_dispatch_globals_text_batch_from_resolver()` —
//! convenience entry point combining plan-building + text-batch generation.
//!
//! **Critical invariant**: output must match calling `emit_vtables()` +
//! `emit_dyn_trait_ptrs()` separately (via Emitter).
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{
    emit_dyn_trait_ptrs, emit_trait_dispatch_globals_text_batch,
    emit_trait_dispatch_globals_text_batch_from_resolver, emit_vtables, TextEmitter,
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

/// Empty TraitResolver → empty Vec.
#[test]
fn test_emit_trait_dispatch_globals_text_batch_from_resolver_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let lines = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);
    assert!(lines.is_empty());
}

// ---------------------------------------------------------------------------
// Single vtable
// ---------------------------------------------------------------------------

/// Single vtable → 2 lines (1 vtable + 1 dynptr).
#[test]
fn test_emit_dispatch_globals_text_batch_from_resolver_single() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let lines = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("@.vtable.Foo.S"));
    assert!(lines[1].starts_with("@.dynptr.Foo.S"));
}

// ---------------------------------------------------------------------------
// Multi vtable
// ---------------------------------------------------------------------------

/// Multiple vtables → multiple vtable + dynptr lines.
#[test]
fn test_emit_dispatch_globals_text_batch_from_resolver_multi() {
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

    let lines = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);
    assert_eq!(lines.len(), 4); // 2 vtable + 2 dynptr
    let vtable_lines: Vec<&String> = lines
        .iter()
        .filter(|l| l.starts_with("@.vtable."))
        .collect();
    let dynptr_lines: Vec<&String> = lines
        .iter()
        .filter(|l| l.starts_with("@.dynptr."))
        .collect();
    assert_eq!(vtable_lines.len(), 2);
    assert_eq!(dynptr_lines.len(), 2);
}

// ---------------------------------------------------------------------------
// **Critical**: matches separate emit_vtables + emit_dyn_trait_ptrs
// ---------------------------------------------------------------------------

/// `emit_trait_dispatch_globals_text_batch_from_resolver()` must produce
/// IR lines that match what `emit_vtables()` + `emit_dyn_trait_ptrs()`
/// emit via Emitter.
#[test]
fn test_match_separate_emit_vtables_and_dyn_trait_ptrs() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_S_clone", "landin_S_clone_from"],
    );

    // Get text batch output (no Emitter)
    let text_lines = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);

    // Get separate emit_vtables + emit_dyn_trait_ptrs output (via Emitter)
    let mut emitter = TextEmitter::new();
    emit_vtables(&resolver, &interner, &mut emitter);
    emit_dyn_trait_ptrs(&resolver, &interner, &mut emitter);
    let emitter_output = emitter.output_with_globals();

    // Each text line should appear in the emitter output
    for line in &text_lines {
        assert!(
            emitter_output.contains(line),
            "text line not found in emitter output.\n\
             text line: {line}\n\
             emitter output: {emitter_output}"
        );
    }
}

// ---------------------------------------------------------------------------
// **Critical**: matches plan-based text batch
// ---------------------------------------------------------------------------

/// Must match `emit_trait_dispatch_globals_text_batch()` when given the plan
/// from the same resolver.
#[test]
fn test_match_plan_based_text_batch() {
    use landin_compiler::codegen::build_trait_dispatch_emission_plan;
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    // From resolver (convenience entry)
    let lines_from_resolver =
        emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);

    // From plan (two-step)
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let lines_from_plan = emit_trait_dispatch_globals_text_batch(&plan);

    // Compare as sets (HashMap order may differ)
    assert_eq!(lines_from_resolver.len(), lines_from_plan.len());
    for line in &lines_from_resolver {
        assert!(
            lines_from_plan.contains(line),
            "line {line} not in plan-based output"
        );
    }
}

// ---------------------------------------------------------------------------
// No side effects
// ---------------------------------------------------------------------------

/// Pure function — doesn't modify input resolver.
#[test]
fn test_no_side_effects_on_resolver() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let vtables_count_before = resolver.vtables.len();

    let _lines = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);

    assert_eq!(resolver.vtables.len(), vtables_count_before);
}

// ---------------------------------------------------------------------------
// No Emitter needed
// ---------------------------------------------------------------------------

/// No Emitter needed — function works without any Emitter trait object.
#[test]
fn test_no_emitter_needed() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let lines = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);
    assert!(!lines.is_empty());
    for line in &lines {
        assert!(line.starts_with("@"));
        assert!(line.contains("internal unnamed_addr constant"));
    }
}

// ---------------------------------------------------------------------------
// Order correctness
// ---------------------------------------------------------------------------

/// Vtable lines appear first.
#[test]
fn test_vtable_lines_first() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let lines = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("@.vtable."));
}

/// Dynptr lines appear second.
#[test]
fn test_dynptr_lines_second() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let lines = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);
    assert_eq!(lines.len(), 2);
    assert!(lines[1].starts_with("@.dynptr."));
}

// ---------------------------------------------------------------------------
// Count + real scenario + determinism
// ---------------------------------------------------------------------------

/// Total line count == 2 × vtables.len().
#[test]
fn test_count_matches_vtables() {
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
    let lines = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);
    assert_eq!(lines.len(), 6); // 3 vtable + 3 dynptr
}

/// Simulate real TraitResolver with multiple (trait, type) pairs.
#[test]
fn test_real_scenario() {
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

    let lines = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);
    assert_eq!(lines.len(), 6);
    for line in &lines {
        assert!(line.starts_with("@"));
    }
}

/// Repeated calls produce identical output (deterministic).
#[test]
fn test_deterministic() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);

    let lines1 = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);
    let lines2 = emit_trait_dispatch_globals_text_batch_from_resolver(&resolver, &interner);

    assert_eq!(lines1, lines2);
}
