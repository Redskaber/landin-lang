//! Stage 5.58: TextEmitter::emit_dyn_trait_const delegation tests
//!
//! Tests that `TextEmitter::emit_dyn_trait_const()` correctly delegates to
//! `emit_dynptr_global_text()` (Stage 5.48 free function).
//!
//! **Critical invariant**: the delegation must produce byte-for-byte identical
//! output to the free function, AND must not regress any existing dynptr
//! codegen behavior.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{
    emit_dyn_trait_ptrs, emit_dynptr_global_text, Emitter, TextEmitter,
};
use landin_compiler::traits::{TraitResolver, Vtable, VtableEntry};
use lasso::Rodeo;

// ---------------------------------------------------------------------------
// Basic delegation correctness
// ---------------------------------------------------------------------------

/// Delegated method produces correct IR for basic case.
#[test]
fn test_text_emitter_dynptr_delegation_basic() {
    let mut emitter = TextEmitter::new();
    let result = emitter.emit_dyn_trait_const(".dynptr.Foo.S", ".data.S", ".vtable.Foo.S");
    let output = emitter.output_with_globals();

    assert_eq!(result, ".dynptr.Foo.S");
    assert!(output.contains("@.dynptr.Foo.S = private unnamed_addr constant"));
    assert!(output.contains("ptr @.data.S"));
    assert!(output.contains("ptr @.vtable.Foo.S"));
}

/// Format verification — full IR line.
#[test]
fn test_text_emitter_dynptr_delegation_format() {
    let mut emitter = TextEmitter::new();
    let _ = emitter.emit_dyn_trait_const(
        ".dynptr.Clone.MyType",
        ".data.MyType",
        ".vtable.Clone.MyType",
    );
    let output = emitter.output_with_globals();

    assert!(output.contains(
        "@.dynptr.Clone.MyType = private unnamed_addr constant \
         { ptr, ptr } { ptr @.data.MyType, ptr @.vtable.Clone.MyType }"
    ));
}

/// Foo + S example.
#[test]
fn test_text_emitter_dynptr_delegation_foo_s() {
    let mut emitter = TextEmitter::new();
    let _ = emitter.emit_dyn_trait_const(".dynptr.Foo.S", ".data.S", ".vtable.Foo.S");
    let output = emitter.output_with_globals();

    assert!(output.starts_with("; ")); // header
    assert!(output.contains("@.dynptr.Foo.S"));
    assert!(output.contains("{ ptr, ptr }"));
}

// ---------------------------------------------------------------------------
// No regression — emit_dyn_trait_ptrs still works
// ---------------------------------------------------------------------------

/// Verify that emit_dyn_trait_ptrs (which calls emit_dyn_trait_const internally)
/// still produces correct output after delegation.
#[test]
fn test_text_emitter_dynptr_delegation_no_regression() {
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
    emit_dyn_trait_ptrs(&resolver, &interner, &mut emitter);
    let output = emitter.output_with_globals();

    // Should still produce correct dynptr global
    assert!(output.contains("@.dynptr.Foo.S"));
    assert!(output.contains("ptr @.data.S"));
    assert!(output.contains("ptr @.vtable.Foo.S"));
}

// ---------------------------------------------------------------------------
// Delegation output == free function output
// ---------------------------------------------------------------------------

/// The delegated method output must match `emit_dynptr_global_text()` output.
#[test]
fn test_text_emitter_dynptr_delegation_match_free_fn() {
    let global_name = ".dynptr.Foo.S";
    let data_symbol = ".data.S";
    let vtable_symbol = ".vtable.Foo.S";

    // Get free function output
    let free_fn_ir = emit_dynptr_global_text(global_name, data_symbol, vtable_symbol);

    // Get TextEmitter output (delegated)
    let mut emitter = TextEmitter::new();
    let _ = emitter.emit_dyn_trait_const(global_name, data_symbol, vtable_symbol);
    let emitter_output = emitter.output_with_globals();

    assert!(
        emitter_output.contains(&free_fn_ir),
        "delegated output should contain free fn IR.\n\
         free fn: {free_fn_ir}\n\
         emitter: {emitter_output}"
    );
}

// ---------------------------------------------------------------------------
// Emitter globals + return value
// ---------------------------------------------------------------------------

/// globals Vec contains the correct entry after delegation.
#[test]
fn test_text_emitter_dynptr_delegation_emitter_globals() {
    let mut emitter = TextEmitter::new();
    let _ = emitter.emit_dyn_trait_const(".dynptr.Foo.S", ".data.S", ".vtable.Foo.S");
    let output = emitter.output_with_globals();

    let dynptr_count = output
        .lines()
        .filter(|l| l.starts_with("@.dynptr.") && l.contains("private unnamed_addr constant"))
        .count();
    assert_eq!(dynptr_count, 1);
}

/// Return value is the global_name (without leading @).
#[test]
fn test_text_emitter_dynptr_delegation_return_value() {
    let mut emitter = TextEmitter::new();
    let result = emitter.emit_dyn_trait_const(".dynptr.Foo.S", ".data.S", ".vtable.Foo.S");
    assert_eq!(result, ".dynptr.Foo.S");
}

/// Data and vtable symbols are correctly placed in the IR.
#[test]
fn test_text_emitter_dynptr_delegation_data_vtable_symbols() {
    let mut emitter = TextEmitter::new();
    let _ = emitter.emit_dyn_trait_const(".dynptr.Display.Vec", ".data.Vec", ".vtable.Display.Vec");
    let output = emitter.output_with_globals();

    assert!(output.contains("ptr @.data.Vec"));
    assert!(output.contains("ptr @.vtable.Display.Vec"));
}

// ---------------------------------------------------------------------------
// Real scenario + multiple
// ---------------------------------------------------------------------------

/// Simulate real dynptr emission scenario (S impls Clone + Drop).
#[test]
fn test_text_emitter_dynptr_delegation_real_scenario() {
    let mut emitter = TextEmitter::new();

    // S impls Clone + Drop → 2 dynptr globals
    let _ = emitter.emit_dyn_trait_const(".dynptr.Clone.S", ".data.S", ".vtable.Clone.S");
    let _ = emitter.emit_dyn_trait_const(".dynptr.Drop.S", ".data.S", ".vtable.Drop.S");

    let output = emitter.output_with_globals();

    let dynptr_count = output
        .lines()
        .filter(|l| l.starts_with("@.dynptr.") && l.contains("private unnamed_addr constant"))
        .count();
    assert_eq!(dynptr_count, 2);
    assert!(output.contains("@.dynptr.Clone.S"));
    assert!(output.contains("@.dynptr.Drop.S"));
    // Both share the same data symbol (.data.S) since same type
    assert!(output.contains("ptr @.data.S"));
}

/// Multiple dynptr globals — verify independence.
#[test]
fn test_text_emitter_dynptr_delegation_multiple() {
    let mut emitter = TextEmitter::new();
    let cases = [
        (".dynptr.A.X", ".data.X", ".vtable.A.X"),
        (".dynptr.B.Y", ".data.Y", ".vtable.B.Y"),
        (".dynptr.C.Z", ".data.Z", ".vtable.C.Z"),
    ];
    for (g, d, v) in cases {
        let _ = emitter.emit_dyn_trait_const(g, d, v);
    }
    let output = emitter.output_with_globals();

    let dynptr_count = output
        .lines()
        .filter(|l| l.starts_with("@.dynptr.") && l.contains("private unnamed_addr constant"))
        .count();
    assert_eq!(dynptr_count, 3);
}
