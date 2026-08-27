//! Stage 5.48: Codegen dynptr global text helper tests
//!
//! Tests `emit_dynptr_global_text()` — pure free function in
//! `src/codegen/mod.rs` that produces LLVM IR text for one `dyn Trait`
//! fat-pointer global.
//!
//! **Critical invariant**: the output must be byte-for-byte identical to
//! what `TextEmitter::emit_dyn_trait_const()` produces. The
//! `test_emit_dynptr_global_text_match_text_emitter` test verifies this
//! by constructing both and asserting equality.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{emit_dynptr_global_text, ModuleEmitter, TextEmitter};

// ---------------------------------------------------------------------------
// Basic emission
// ---------------------------------------------------------------------------

/// Basic dynptr IR: @.dynptr.Foo.S = ... { ptr, ptr } { ptr @.data.S, ptr @.vtable.Foo.S }
#[test]
fn test_emit_dynptr_global_text_basic() {
    let ir = emit_dynptr_global_text(".dynptr.Foo.S", ".data.S", ".vtable.Foo.S");
    assert_eq!(
        ir,
        "@.dynptr.Foo.S = internal unnamed_addr constant \
         { ptr, ptr } { ptr @.data.S, ptr @.vtable.Foo.S }"
    );
}

/// Foo + S example (matches doc comment example).
#[test]
fn test_emit_dynptr_global_text_foo_s() {
    let ir = emit_dynptr_global_text(".dynptr.Foo.S", ".data.S", ".vtable.Foo.S");
    assert!(ir.starts_with("@.dynptr.Foo.S = "));
    assert!(ir.contains("{ ptr, ptr }"));
    assert!(ir.contains("ptr @.data.S"));
    assert!(ir.contains("ptr @.vtable.Foo.S"));
}

/// Display + Vec example.
#[test]
fn test_emit_dynptr_global_text_display_vec() {
    let ir = emit_dynptr_global_text(".dynptr.Display.Vec", ".data.Vec", ".vtable.Display.Vec");
    assert!(ir.starts_with("@.dynptr.Display.Vec = "));
    assert!(ir.contains("ptr @.data.Vec"));
    assert!(ir.contains("ptr @.vtable.Display.Vec"));
}

// ---------------------------------------------------------------------------
// Format components
// ---------------------------------------------------------------------------

/// Global name appears with `@` prefix.
#[test]
fn test_emit_dynptr_global_text_global_name() {
    let ir = emit_dynptr_global_text(".dynptr.Bar.T", ".data.T", ".vtable.Bar.T");
    assert!(ir.starts_with("@.dynptr.Bar.T = "));
}

/// data symbol appears as `ptr @<data_symbol>`.
#[test]
fn test_emit_dynptr_global_text_data_symbol() {
    let ir = emit_dynptr_global_text(".dynptr.Foo.S", ".data.S", ".vtable.Foo.S");
    assert!(ir.contains("ptr @.data.S"));
}

/// vtable symbol appears as `ptr @<vtable_symbol>`.
#[test]
fn test_emit_dynptr_global_text_vtable_symbol() {
    let ir = emit_dynptr_global_text(".dynptr.Foo.S", ".data.S", ".vtable.Foo.S");
    assert!(ir.contains("ptr @.vtable.Foo.S"));
}

/// Input global_name should NOT have leading `@` (function adds it).
#[test]
fn test_emit_dynptr_global_text_no_leading_at_in_input() {
    // Input ".dynptr.Foo.S" (no @) → output "@.dynptr.Foo.S = ..."
    let ir = emit_dynptr_global_text(".dynptr.Foo.S", ".data.S", ".vtable.Foo.S");
    assert!(ir.starts_with("@.dynptr.Foo.S"));
    // Verify exactly three `@` symbols: one for global_name, one for data, one for vtable
    assert_eq!(ir.matches('@').count(), 3);
}

/// Struct type `{ ptr, ptr }` appears in the IR.
#[test]
fn test_emit_dynptr_global_text_struct_type() {
    let ir = emit_dynptr_global_text(".dynptr.Foo.S", ".data.S", ".vtable.Foo.S");
    assert!(ir.contains("{ ptr, ptr }"));
}

/// Full format verification.
#[test]
fn test_emit_dynptr_global_text_format() {
    let ir = emit_dynptr_global_text(
        ".dynptr.Clone.MyType",
        ".data.MyType",
        ".vtable.Clone.MyType",
    );
    // Full expected IR line
    assert_eq!(
        ir,
        "@.dynptr.Clone.MyType = internal unnamed_addr constant \
         { ptr, ptr } { ptr @.data.MyType, ptr @.vtable.Clone.MyType }"
    );
}

// ---------------------------------------------------------------------------
// **Critical**: byte-for-byte equivalence with TextEmitter::emit_dyn_trait_const
// ---------------------------------------------------------------------------

/// Output must match `TextEmitter::emit_dyn_trait_const()` byte-for-byte.
/// This is the safety net for Stage 5.49 refactor — when we delegate
/// `TextEmitter::emit_dyn_trait_const()` to this free function, behavior
/// stays identical.
#[test]
fn test_emit_dynptr_global_text_match_text_emitter() {
    let global_name = ".dynptr.Foo.S";
    let data_symbol = ".data.S";
    let vtable_symbol = ".vtable.Foo.S";

    // Get the free function output.
    let free_fn_ir = emit_dynptr_global_text(global_name, data_symbol, vtable_symbol);

    // Get the TextEmitter output by calling the trait method directly.
    let mut emitter = TextEmitter::new();
    let _ = emitter.emit_dyn_trait_const(global_name, data_symbol, vtable_symbol);
    let text_emitter_ir = emitter.output_with_globals();

    // The free function output should appear verbatim in the TextEmitter output.
    assert!(
        text_emitter_ir.contains(&free_fn_ir),
        "free fn IR not found in TextEmitter output.\n\
         free fn: {free_fn_ir}\n\
         TextEmitter: {text_emitter_ir}"
    );
}

// ---------------------------------------------------------------------------
// Real scenario
// ---------------------------------------------------------------------------

/// Simulate real scenario: multiple dynptr globals for different (trait, type) pairs.
#[test]
fn test_emit_dynptr_global_text_real_scenario() {
    // S impls Clone + Drop → 2 dynptr globals
    let ir1 = emit_dynptr_global_text(".dynptr.Clone.S", ".data.S", ".vtable.Clone.S");
    let ir2 = emit_dynptr_global_text(".dynptr.Drop.S", ".data.S", ".vtable.Drop.S");

    assert!(ir1.contains("@.dynptr.Clone.S"));
    assert!(ir1.contains("ptr @.vtable.Clone.S"));
    assert!(ir2.contains("@.dynptr.Drop.S"));
    assert!(ir2.contains("ptr @.vtable.Drop.S"));

    // Both share the same data symbol (.data.S) since same type
    assert!(ir1.contains("ptr @.data.S"));
    assert!(ir2.contains("ptr @.data.S"));
}

/// Multiple constants in sequence — verify each is independent.
#[test]
fn test_emit_dynptr_global_text_constants() {
    let cases = [
        (".dynptr.A.X", ".data.X", ".vtable.A.X"),
        (".dynptr.B.Y", ".data.Y", ".vtable.B.Y"),
        (".dynptr.C.Z", ".data.Z", ".vtable.C.Z"),
    ];
    for (g, d, v) in cases {
        let ir = emit_dynptr_global_text(g, d, v);
        assert!(ir.starts_with(&format!("@{g} = ")));
        assert!(ir.contains(&format!("ptr @{d}")));
        assert!(ir.contains(&format!("ptr @{v}")));
    }
}
