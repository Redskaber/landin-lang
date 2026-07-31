//! Stage 5.55: Codegen trait-dispatch emission text batch tests
//!
//! Tests `emit_trait_dispatch_globals_text_batch()` — plan-based text batch
//! that generates all trait-dispatch globals (vtable + dynptr) LLVM IR text
//! WITHOUT needing an Emitter.
//!
//! **Critical invariant**: the IR text lines must match what
//! `emit_trait_dispatch_globals_from_plan()` (Stage 5.54) emits.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{
    build_trait_dispatch_emission_plan, emit_trait_dispatch_globals_from_plan,
    emit_trait_dispatch_globals_text_batch, TextEmitter,
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

/// Empty plan → empty Vec.
#[test]
fn test_emit_trait_dispatch_globals_text_batch_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);

    let lines = emit_trait_dispatch_globals_text_batch(&plan);
    assert!(lines.is_empty());
}

// ---------------------------------------------------------------------------
// Single vtable
// ---------------------------------------------------------------------------

/// Single vtable → vtable + dynptr IR line.
#[test]
fn test_emit_trait_dispatch_globals_text_batch_single() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);

    let lines = emit_trait_dispatch_globals_text_batch(&plan);
    assert_eq!(lines.len(), 2); // 1 vtable + 1 dynptr
    assert!(lines[0].starts_with("@.vtable.Foo.S"));
    assert!(lines[1].starts_with("@.dynptr.Foo.S"));
}

// ---------------------------------------------------------------------------
// Multi vtable
// ---------------------------------------------------------------------------

/// Multiple vtables → multiple vtable + dynptr IR lines.
#[test]
fn test_emit_trait_dispatch_globals_text_batch_multi() {
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

    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let lines = emit_trait_dispatch_globals_text_batch(&plan);
    assert_eq!(lines.len(), 4); // 2 vtable + 2 dynptr

    // Check that all 4 globals appear (order: vtable specs first, then dynptr specs)
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
// **Critical**: matches orchestrator output
// ---------------------------------------------------------------------------

/// The IR text lines must match what `emit_trait_dispatch_globals_from_plan()`
/// (Stage 5.54) emits via the Emitter.
#[test]
fn test_emit_trait_dispatch_globals_text_batch_match_orchestrator() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_S_clone", "landin_S_clone_from"],
    );
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);

    // Get text batch output (no Emitter)
    let text_lines = emit_trait_dispatch_globals_text_batch(&plan);

    // Get orchestrator output (via Emitter)
    let mut emitter = TextEmitter::new();
    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter);
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
// No side effects
// ---------------------------------------------------------------------------

/// Pure function — doesn't modify input plan.
#[test]
fn test_emit_trait_dispatch_globals_text_batch_no_side_effects() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let vtable_specs_count_before = plan.vtable_specs.len();

    let _lines = emit_trait_dispatch_globals_text_batch(&plan);

    assert_eq!(plan.vtable_specs.len(), vtable_specs_count_before);
}

// ---------------------------------------------------------------------------
// Line correctness
// ---------------------------------------------------------------------------

/// Vtable IR lines are correct.
#[test]
fn test_emit_trait_dispatch_globals_text_batch_vtable_lines() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_S_drop"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);

    let lines = emit_trait_dispatch_globals_text_batch(&plan);
    let vtable_line = lines.iter().find(|l| l.starts_with("@.vtable.")).unwrap();
    assert!(vtable_line.contains("@.vtable.Drop.S = private unnamed_addr constant"));
    assert!(vtable_line.contains("ptr @landin_S_drop"));
}

/// Dynptr IR lines are correct.
#[test]
fn test_emit_trait_dispatch_globals_text_batch_dynptr_lines() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_S_drop"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);

    let lines = emit_trait_dispatch_globals_text_batch(&plan);
    let dynptr_line = lines.iter().find(|l| l.starts_with("@.dynptr.")).unwrap();
    assert!(dynptr_line.contains("@.dynptr.Drop.S = private unnamed_addr constant"));
    assert!(dynptr_line.contains("ptr @.data.S"));
    assert!(dynptr_line.contains("ptr @.vtable.Drop.S"));
}

/// Total line count == 2 × specs.len() (vtable + dynptr per spec).
#[test]
fn test_emit_trait_dispatch_globals_text_batch_count_matches() {
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

    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let lines = emit_trait_dispatch_globals_text_batch(&plan);
    // 3 vtable + 3 dynptr = 6 lines
    assert_eq!(lines.len(), 6);
}

/// vtable lines appear before dynptr lines (order).
#[test]
fn test_emit_trait_dispatch_globals_text_batch_order() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);

    let lines = emit_trait_dispatch_globals_text_batch(&plan);
    assert_eq!(lines.len(), 2);
    // First line is vtable, second is dynptr
    assert!(lines[0].starts_with("@.vtable."));
    assert!(lines[1].starts_with("@.dynptr."));
}

// ---------------------------------------------------------------------------
// Real scenario + no Emitter + determinism
// ---------------------------------------------------------------------------

/// Simulate real TraitResolver with multiple (trait, type) pairs.
#[test]
fn test_emit_trait_dispatch_globals_text_batch_real_scenario() {
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

    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let lines = emit_trait_dispatch_globals_text_batch(&plan);
    // 3 vtable + 3 dynptr = 6 lines
    assert_eq!(lines.len(), 6);
    // All lines start with @
    for line in &lines {
        assert!(line.starts_with("@"));
    }
}

/// No Emitter needed — function works without any Emitter trait object.
#[test]
fn test_emit_trait_dispatch_globals_text_batch_no_emitter_needed() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);

    // Call without any Emitter — just get Vec<String>
    let lines = emit_trait_dispatch_globals_text_batch(&plan);
    assert!(!lines.is_empty());
    // Verify the lines are valid LLVM IR (start with @, contain "constant")
    for line in &lines {
        assert!(line.starts_with("@"));
        assert!(line.contains("private unnamed_addr constant"));
    }
}

/// Repeated calls produce identical output (deterministic).
#[test]
fn test_emit_trait_dispatch_globals_text_batch_deterministic() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);

    let lines1 = emit_trait_dispatch_globals_text_batch(&plan);
    let lines2 = emit_trait_dispatch_globals_text_batch(&plan);

    assert_eq!(lines1, lines2);
}
