//! Stage 5.70: emit_dyn_trait_method_calls_text_batch_from_resolver tests
//!
//! Tests the convenience entry point that combines Stage 5.62 + 5.68 + 5.69.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::dyn_trait_emit::emit_dyn_trait_method_calls_text_batch_from_resolver;
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

/// Empty TraitResolver → empty Vec.
#[test]
fn test_method_calls_from_resolver_empty() {
    let resolver = TraitResolver::new();
    let interner = Rodeo::new();
    let lines = emit_dyn_trait_method_calls_text_batch_from_resolver(&resolver, &interner);
    assert!(lines.is_empty());
}

/// Single vtable (Clone) → 2 method call IR blocks.
#[test]
fn test_method_calls_from_resolver_clone() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Clone", "S", &["landin_S_clone"]);
    let lines = emit_dyn_trait_method_calls_text_batch_from_resolver(&resolver, &interner);
    // Clone has 2 methods (clone + clone_from) in stdlib registry
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("; dyn Clone.S::clone"));
    assert!(lines[1].contains("; dyn Clone.S::clone_from"));
}

/// Drop → 1 method call.
#[test]
fn test_method_calls_from_resolver_drop() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "S", &["landin_S_drop"]);
    let lines = emit_dyn_trait_method_calls_text_batch_from_resolver(&resolver, &interner);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("; dyn Drop.S::drop"));
}

/// No side effects on resolver.
#[test]
fn test_method_calls_from_resolver_no_side_effects() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Clone", "S", &["landin_S_clone"]);
    let count_before = resolver.vtables.len();
    let _ = emit_dyn_trait_method_calls_text_batch_from_resolver(&resolver, &interner);
    assert_eq!(resolver.vtables.len(), count_before);
}

/// Real scenario: S impls Clone + Drop + Display.
#[test]
fn test_method_calls_from_resolver_real_scenario() {
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
    let lines = emit_dyn_trait_method_calls_text_batch_from_resolver(&resolver, &interner);
    // Clone(2) + Drop(1) + Display(1) = 4
    assert_eq!(lines.len(), 4);
    for line in &lines {
        assert!(line.contains("getelementptr"));
        assert!(line.contains("call ptr"));
    }
}

/// Deterministic — repeated calls identical.
#[test]
fn test_method_calls_from_resolver_deterministic() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Clone", "S", &["landin_S_clone"]);
    let l1 = emit_dyn_trait_method_calls_text_batch_from_resolver(&resolver, &interner);
    let l2 = emit_dyn_trait_method_calls_text_batch_from_resolver(&resolver, &interner);
    assert_eq!(l1, l2);
}

/// All lines are valid LLVM IR.
#[test]
fn test_method_calls_from_resolver_valid_ir() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Drop", "Vec", &["landin_Vec_drop"]);
    let lines = emit_dyn_trait_method_calls_text_batch_from_resolver(&resolver, &interner);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("; dyn Drop.Vec::drop"));
    assert!(lines[0].contains("getelementptr"));
    assert!(lines[0].contains("load ptr"));
    assert!(lines[0].contains("call ptr"));
}

/// No Emitter needed.
#[test]
fn test_method_calls_from_resolver_no_emitter() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_vtable(&mut interner, "Clone", "S", &["landin_S_clone"]);
    let lines = emit_dyn_trait_method_calls_text_batch_from_resolver(&resolver, &interner);
    assert!(!lines.is_empty());
}
