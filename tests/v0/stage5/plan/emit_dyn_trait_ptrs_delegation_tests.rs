//! Stage 5.60: emit_dyn_trait_ptrs delegation tests
//!
//! Tests that `emit_dyn_trait_ptrs()` correctly delegates to
//! `emit_dynptrs_from_resolver()` (Stage 5.50). Fourth and final
//! existing-path modification.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{emit_dyn_trait_ptrs, emit_dynptrs_from_resolver, TextEmitter};
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

/// Delegated emit_dyn_trait_ptrs produces correct IR.
#[test]
fn test_emit_dyn_trait_ptrs_delegation_basic() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let mut emitter = TextEmitter::new();
    emit_dyn_trait_ptrs(&resolver, &interner, &mut emitter);
    let output = emitter.output_with_globals();
    assert!(output.contains("@.dynptr.Foo.S"));
    assert!(output.contains("ptr @.data.S"));
    assert!(output.contains("ptr @.vtable.Foo.S"));
}

/// Empty TraitResolver → no dynptr globals.
#[test]
fn test_emit_dyn_trait_ptrs_delegation_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let mut emitter = TextEmitter::new();
    emit_dyn_trait_ptrs(&resolver, &interner, &mut emitter);
    let output = emitter.output_with_globals();
    assert!(!output.contains("@.dynptr."));
}

/// Single vtable → 1 dynptr global.
#[test]
fn test_emit_dyn_trait_ptrs_delegation_single() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_S_drop"]);
    let mut emitter = TextEmitter::new();
    emit_dyn_trait_ptrs(&resolver, &interner, &mut emitter);
    let output = emitter.output_with_globals();
    let count = output
        .lines()
        .filter(|l| l.starts_with("@.dynptr.") && l.contains("constant"))
        .count();
    assert_eq!(count, 1);
}

/// Multiple vtables → multiple dynptr globals.
#[test]
fn test_emit_dyn_trait_ptrs_delegation_multi() {
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
    let mut emitter = TextEmitter::new();
    emit_dyn_trait_ptrs(&resolver, &interner, &mut emitter);
    let output = emitter.output_with_globals();
    let count = output
        .lines()
        .filter(|l| l.starts_with("@.dynptr.") && l.contains("constant"))
        .count();
    assert_eq!(count, 2);
}

/// emit_dyn_trait_ptrs (delegated) == emit_dynptrs_from_resolver (Stage 5.50).
#[test]
fn test_emit_dyn_trait_ptrs_delegation_match_orchestrator() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(
        &mut interner,
        "Clone",
        "S",
        &["landin_S_clone", "landin_S_clone_from"],
    );
    let mut emitter1 = TextEmitter::new();
    emit_dyn_trait_ptrs(&resolver, &interner, &mut emitter1);
    let output1 = emitter1.output_with_globals();

    let mut emitter2 = TextEmitter::new();
    emit_dynptrs_from_resolver(&resolver, &interner, &mut emitter2);
    let output2 = emitter2.output_with_globals();

    assert_eq!(output1, output2);
}

/// Real scenario: S impls Clone + Drop + Display.
#[test]
fn test_emit_dyn_trait_ptrs_delegation_real_scenario() {
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
    let mut emitter = TextEmitter::new();
    emit_dyn_trait_ptrs(&resolver, &interner, &mut emitter);
    let output = emitter.output_with_globals();
    let count = output
        .lines()
        .filter(|l| l.starts_with("@.dynptr.") && l.contains("constant"))
        .count();
    assert_eq!(count, 3);
}

/// Repeated calls produce identical output.
#[test]
fn test_emit_dyn_trait_ptrs_delegation_deterministic() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Foo", "S", &["landin_S_bar"]);
    let mut emitter1 = TextEmitter::new();
    emit_dyn_trait_ptrs(&resolver, &interner, &mut emitter1);
    let output1 = emitter1.output_with_globals();

    let mut emitter2 = TextEmitter::new();
    emit_dyn_trait_ptrs(&resolver, &interner, &mut emitter2);
    let output2 = emitter2.output_with_globals();

    assert_eq!(output1, output2);
}
