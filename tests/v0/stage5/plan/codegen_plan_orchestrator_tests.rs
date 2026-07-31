//! Stage 5.54: Codegen trait-dispatch emission orchestrator (plan-based) tests
//!
//! Tests `emit_trait_dispatch_globals_from_plan()` — first plan-based
//! orchestrator that emits all trait-dispatch globals from a
//! `CodegenTraitDispatchEmissionPlan`.
//!
//! **Critical invariant**: behavior must be identical to
//! `emit_vtables_and_dynptrs_from_resolver()` (Stage 5.51) when given the
//! plan from the same resolver.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{
    build_trait_dispatch_emission_plan, emit_trait_dispatch_globals_from_plan,
    emit_vtables_and_dynptrs_from_resolver, TextEmitter,
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

/// Empty plan → no emitter calls, empty output.
#[test]
fn test_emit_trait_dispatch_globals_from_plan_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let mut emitter = TextEmitter::new();

    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(!output.contains(".vtable."));
    assert!(!output.contains(".dynptr."));
}

// ---------------------------------------------------------------------------
// Single vtable
// ---------------------------------------------------------------------------

/// Single vtable → vtable + dynptr global.
#[test]
fn test_emit_trait_dispatch_globals_from_plan_single() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let mut emitter = TextEmitter::new();

    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(output.contains("@.vtable.Foo.S"));
    assert!(output.contains("@.dynptr.Foo.S"));
}

// ---------------------------------------------------------------------------
// Multi vtable
// ---------------------------------------------------------------------------

/// Multiple vtables → multiple vtable + dynptr globals.
#[test]
fn test_emit_trait_dispatch_globals_from_plan_multi() {
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
    let mut emitter = TextEmitter::new();
    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(output.contains("@.vtable.Foo.S"));
    assert!(output.contains("@.vtable.Bar.T"));
    assert!(output.contains("@.dynptr.Foo.S"));
    assert!(output.contains("@.dynptr.Bar.T"));
}

// ---------------------------------------------------------------------------
// **Critical**: matches resolver-based orchestrator
// ---------------------------------------------------------------------------

/// `emit_trait_dispatch_globals_from_plan()` must produce identical output
/// to `emit_vtables_and_dynptrs_from_resolver()` (Stage 5.51) when given
/// the plan from the same resolver.
#[test]
fn test_emit_trait_dispatch_globals_from_plan_match_resolver_orchestrator() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_S_clone", "landin_S_clone_from"],
    );

    // Call resolver-based orchestrator (Stage 5.51)
    let mut emitter1 = TextEmitter::new();
    emit_vtables_and_dynptrs_from_resolver(&resolver, &interner, &mut emitter1);
    let output1 = emitter1.output_with_globals();

    // Call plan-based orchestrator (Stage 5.54)
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let mut emitter2 = TextEmitter::new();
    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter2);
    let output2 = emitter2.output_with_globals();

    // Outputs must be identical
    assert_eq!(
        output1, output2,
        "plan-based orchestrator output differs from resolver-based.\n\
         resolver-based: {output1}\n\
         plan-based: {output2}"
    );
}

// ---------------------------------------------------------------------------
// No side effects on plan
// ---------------------------------------------------------------------------

/// Pure with respect to plan — doesn't modify it.
#[test]
fn test_emit_trait_dispatch_globals_from_plan_no_side_effects_on_plan() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let vtable_specs_count_before = plan.vtable_specs.len();
    let dynptr_specs_count_before = plan.dynptr_specs.len();
    let mut emitter = TextEmitter::new();

    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter);

    assert_eq!(plan.vtable_specs.len(), vtable_specs_count_before);
    assert_eq!(plan.dynptr_specs.len(), dynptr_specs_count_before);
}

// ---------------------------------------------------------------------------
// Emission correctness
// ---------------------------------------------------------------------------

/// Vtable globals are emitted.
#[test]
fn test_emit_trait_dispatch_globals_from_plan_vtable_emitted() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_S_drop"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let mut emitter = TextEmitter::new();

    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(output.contains("@.vtable.Drop.S = private unnamed_addr constant"));
    assert!(output.contains("ptr @landin_S_drop"));
}

/// Dynptr globals are emitted.
#[test]
fn test_emit_trait_dispatch_globals_from_plan_dynptr_emitted() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_S_drop"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let mut emitter = TextEmitter::new();

    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter);

    let output = emitter.output_with_globals();
    assert!(output.contains("@.dynptr.Drop.S = private unnamed_addr constant"));
    assert!(output.contains("ptr @.data.S"));
    assert!(output.contains("ptr @.vtable.Drop.S"));
}

/// Total global count == 2 × specs.len() (vtable + dynptr per spec).
#[test]
fn test_emit_trait_dispatch_globals_from_plan_count_matches() {
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
    let mut emitter = TextEmitter::new();
    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter);

    let output = emitter.output_with_globals();
    // Count global *definitions* (lines starting with @.vtable. or @.dynptr.
    // + "private unnamed_addr constant"), not references inside dynptr initializers.
    let vtable_defs = output
        .lines()
        .filter(|line| {
            line.starts_with("@.vtable.") && line.contains("private unnamed_addr constant")
        })
        .count();
    let dynptr_defs = output
        .lines()
        .filter(|line| {
            line.starts_with("@.dynptr.") && line.contains("private unnamed_addr constant")
        })
        .count();
    assert_eq!(vtable_defs, 3);
    assert_eq!(dynptr_defs, 3);
}

/// vtable globals appear before dynptr globals (order from composition).
#[test]
fn test_emit_trait_dispatch_globals_from_plan_order() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let mut emitter = TextEmitter::new();

    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter);

    let output = emitter.output_with_globals();
    let vtable_pos = output.find("@.vtable.Foo.S");
    let dynptr_pos = output.find("@.dynptr.Foo.S");
    assert!(vtable_pos.is_some());
    assert!(dynptr_pos.is_some());
    assert!(
        vtable_pos.unwrap() < dynptr_pos.unwrap(),
        "vtable should appear before dynptr in output"
    );
}

// ---------------------------------------------------------------------------
// Real scenario + composition + determinism
// ---------------------------------------------------------------------------

/// Simulate real TraitResolver with multiple (trait, type) pairs.
#[test]
fn test_emit_trait_dispatch_globals_from_plan_real_scenario() {
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
    let mut emitter = TextEmitter::new();
    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter);

    let output = emitter.output_with_globals();
    // 3 vtable + 3 dynptr globals
    let vtable_defs = output
        .lines()
        .filter(|line| {
            line.starts_with("@.vtable.") && line.contains("private unnamed_addr constant")
        })
        .count();
    let dynptr_defs = output
        .lines()
        .filter(|line| {
            line.starts_with("@.dynptr.") && line.contains("private unnamed_addr constant")
        })
        .count();
    assert_eq!(vtable_defs, 3);
    assert_eq!(dynptr_defs, 3);
    assert!(output.contains("@.vtable.Clone.S"));
    assert!(output.contains("@.dynptr.Clone.S"));
    assert!(output.contains("@.vtable.Drop.S"));
    assert!(output.contains("@.dynptr.Drop.S"));
}

/// Orchestrator composes plan + emit.
#[test]
fn test_emit_trait_dispatch_globals_from_plan_composes_plan_and_emit() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);
    let mut emitter = TextEmitter::new();

    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter);

    let output = emitter.output_with_globals();
    // Both vtable and dynptr globals present
    assert!(output.contains("@.vtable.Foo.S"));
    assert!(output.contains("@.dynptr.Foo.S"));
}

/// Repeated calls produce identical output (deterministic count).
#[test]
fn test_emit_trait_dispatch_globals_from_plan_deterministic_count() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let plan = build_trait_dispatch_emission_plan(&resolver, &interner);

    let mut emitter1 = TextEmitter::new();
    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter1);
    let output1 = emitter1.output_with_globals();

    let mut emitter2 = TextEmitter::new();
    emit_trait_dispatch_globals_from_plan(&plan, &mut emitter2);
    let output2 = emitter2.output_with_globals();

    let vtable_count1 = output1.matches("@.vtable.").count();
    let vtable_count2 = output2.matches("@.vtable.").count();
    let dynptr_count1 = output1.matches("@.dynptr.").count();
    let dynptr_count2 = output2.matches("@.dynptr.").count();
    assert_eq!(vtable_count1, vtable_count2);
    assert_eq!(dynptr_count1, dynptr_count2);
}
