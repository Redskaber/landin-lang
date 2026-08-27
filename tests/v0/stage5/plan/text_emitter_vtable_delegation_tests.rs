//! Stage 5.57: TextEmitter::emit_vtable_global delegation tests
//!
//! Tests that `TextEmitter::emit_vtable_global()` correctly delegates to
//! `emit_vtable_global_text()` (Stage 5.44 free function) — the first
//! existing-path modification in Stage 5.
//!
//! **Critical invariant**: the delegation must produce byte-for-byte identical
//! output to the free function, AND must not regress any existing vtable
//! codegen behavior.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{emit_vtable_global_text, ModuleEmitter, TextEmitter};

// ---------------------------------------------------------------------------
// Basic delegation correctness
// ---------------------------------------------------------------------------

/// Delegated method produces correct IR for basic case.
#[test]
fn test_text_emitter_vtable_global_delegation_basic() {
    let mut emitter = TextEmitter::new();
    let symbols = vec!["landin_S_bar".to_string()];
    let result = emitter.emit_vtable_global(".vtable.Foo.S", &symbols);
    let output = emitter.output_with_globals();

    assert_eq!(result, ".vtable.Foo.S");
    assert!(output.contains("@.vtable.Foo.S = internal unnamed_addr constant"));
    assert!(output.contains("ptr @landin_S_bar"));
}

/// Empty symbols → zeroinitializer (delegated correctly).
#[test]
fn test_text_emitter_vtable_global_delegation_empty() {
    let mut emitter = TextEmitter::new();
    let result = emitter.emit_vtable_global(".vtable.Copy.S", &[]);
    let output = emitter.output_with_globals();

    assert_eq!(result, ".vtable.Copy.S");
    assert!(output.contains("zeroinitializer"));
}

/// Single symbol delegation.
#[test]
fn test_text_emitter_vtable_global_delegation_single() {
    let mut emitter = TextEmitter::new();
    let symbols = vec!["landin_S_drop".to_string()];
    let _ = emitter.emit_vtable_global(".vtable.Drop.S", &symbols);
    let output = emitter.output_with_globals();

    assert!(output.contains(
        "@.vtable.Drop.S = internal unnamed_addr constant [1 x ptr] [ptr @landin_S_drop]"
    ));
}

/// Multiple symbols delegation.
#[test]
fn test_text_emitter_vtable_global_delegation_multi() {
    let mut emitter = TextEmitter::new();
    let symbols = vec![
        "landin_S_clone".to_string(),
        "landin_S_clone_from".to_string(),
    ];
    let _ = emitter.emit_vtable_global(".vtable.Clone.S", &symbols);
    let output = emitter.output_with_globals();

    assert!(output.contains("[2 x ptr]"));
    assert!(output.contains("ptr @landin_S_clone"));
    assert!(output.contains("ptr @landin_S_clone_from"));
}

// ---------------------------------------------------------------------------
// Null handling bug fix
// ---------------------------------------------------------------------------

/// "null" symbol → `ptr null` (NOT `ptr @null` — the old inline code had this bug).
/// This is the latent bug fix from Stage 5.57 delegation.
#[test]
fn test_text_emitter_vtable_global_delegation_null() {
    let mut emitter = TextEmitter::new();
    let symbols = vec!["landin_S_clone".to_string(), "null".to_string()];
    let _ = emitter.emit_vtable_global(".vtable.Clone.S", &symbols);
    let output = emitter.output_with_globals();

    // The delegated free function correctly emits `ptr null` (not `ptr @null`)
    assert!(output.contains("ptr null"));
    assert!(!output.contains("ptr @null"));
}

// ---------------------------------------------------------------------------
// No regression — existing vtable codegen still works
// ---------------------------------------------------------------------------

/// Verify that emit_vtables (which calls emit_vtable_global internally) still
/// produces correct output after delegation.
#[test]
fn test_text_emitter_vtable_global_delegation_no_regression() {
    use landin_compiler::codegen::emit_vtables;
    use landin_compiler::traits::{TraitResolver, Vtable, VtableEntry};
    use lasso::Rodeo;

    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::new();
    let trait_spur = interner.get_or_intern("Foo");
    let type_spur = interner.get_or_intern("S");
    resolver.vtables.insert(
        (trait_spur, type_spur),
        Vtable {
            trait_name: trait_spur,
            self_ty_name: type_spur,
            impl_def_id: landin_compiler::hir::DefId::new(0),
            entries: vec![VtableEntry {
                method_name: interner.get_or_intern("bar"),
                fn_name: interner.get_or_intern("landin_S_bar"),
            }],
        },
    );

    let mut emitter = TextEmitter::new();
    emit_vtables(&resolver, &interner, &mut emitter);
    let output = emitter.output_with_globals();

    // Should still produce correct vtable global
    assert!(output.contains("@.vtable.Foo.S"));
    assert!(output.contains("ptr @landin_S_bar"));
}

// ---------------------------------------------------------------------------
// Delegation output == free function output
// ---------------------------------------------------------------------------

/// The delegated method output must match `emit_vtable_global_text()` output.
#[test]
fn test_text_emitter_vtable_global_delegation_match_free_fn() {
    let global_name = ".vtable.Foo.S";
    let symbols = vec!["landin_S_bar".to_string(), "landin_S_baz".to_string()];

    // Get free function output
    let free_fn_ir = emit_vtable_global_text(global_name, &symbols);

    // Get TextEmitter output (delegated)
    let mut emitter = TextEmitter::new();
    let _ = emitter.emit_vtable_global(global_name, &symbols);
    let emitter_output = emitter.output_with_globals();

    assert!(
        emitter_output.contains(&free_fn_ir),
        "delegated output should contain free fn IR.\n\
         free fn: {free_fn_ir}\n\
         emitter: {emitter_output}"
    );
}

// ---------------------------------------------------------------------------
// Emitter globals Vec + return value
// ---------------------------------------------------------------------------

/// globals Vec contains the correct entry after delegation.
#[test]
fn test_text_emitter_vtable_global_delegation_emitter_globals() {
    let mut emitter = TextEmitter::new();
    let symbols = vec!["landin_S_bar".to_string()];
    let _ = emitter.emit_vtable_global(".vtable.Foo.S", &symbols);
    let output = emitter.output_with_globals();

    // The globals section should contain exactly one vtable global
    let vtable_count = output
        .lines()
        .filter(|l| l.starts_with("@.vtable.") && l.contains("internal unnamed_addr constant"))
        .count();
    assert_eq!(vtable_count, 1);
}

/// Return value is the global_name (without leading @).
#[test]
fn test_text_emitter_vtable_global_delegation_return_value() {
    let mut emitter = TextEmitter::new();
    let symbols = vec!["landin_S_bar".to_string()];
    let result = emitter.emit_vtable_global(".vtable.Foo.S", &symbols);

    assert_eq!(result, ".vtable.Foo.S");
}

// ---------------------------------------------------------------------------
// Real scenario
// ---------------------------------------------------------------------------

/// Simulate real vtable emission scenario.
#[test]
fn test_text_emitter_vtable_global_delegation_real_scenario() {
    let mut emitter = TextEmitter::new();

    // Emit multiple vtables (simulating S impls Clone + Drop + Display)
    let cases = [
        (
            ".vtable.Clone.S",
            vec!["landin_S_clone", "landin_S_clone_from"],
        ),
        (".vtable.Drop.S", vec!["landin_S_drop"]),
        (".vtable.Display.S", vec!["landin_S_fmt"]),
    ];

    for (name, methods) in &cases {
        let symbols: Vec<String> = methods.iter().map(|s| s.to_string()).collect();
        let _ = emitter.emit_vtable_global(name, &symbols);
    }

    let output = emitter.output_with_globals();

    // All 3 vtables should be present
    let vtable_count = output
        .lines()
        .filter(|l| l.starts_with("@.vtable.") && l.contains("internal unnamed_addr constant"))
        .count();
    assert_eq!(vtable_count, 3);

    // Verify each vtable's content
    assert!(output.contains("@.vtable.Clone.S"));
    assert!(output.contains("ptr @landin_S_clone"));
    assert!(output.contains("ptr @landin_S_clone_from"));
    assert!(output.contains("@.vtable.Drop.S"));
    assert!(output.contains("ptr @landin_S_drop"));
    assert!(output.contains("@.vtable.Display.S"));
    assert!(output.contains("ptr @landin_S_fmt"));
}
