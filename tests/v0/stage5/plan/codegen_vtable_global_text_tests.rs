//! Stage 5.44: Codegen vtable global text bridge tests
//!
//! Tests `emit_vtable_global_text()` — bridge free function in
//! `src/codegen/mod.rs` with the **exact same parameter signature** as
//! `TextEmitter::emit_vtable_global()`.
//!
//! **Critical invariant**: the output must be byte-for-byte identical to
//! what `TextEmitter::emit_vtable_global()` produces on non-null paths.
//! The `test_emit_vtable_global_text_match_text_emitter*` tests verify
//! this by constructing both and asserting equality.
//!
//! Per §16: tests use the public API only (no driver/hir/mir access).
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{emit_vtable_global_text, ModuleEmitter, TextEmitter};

// ---------------------------------------------------------------------------
// Basic emission
// ---------------------------------------------------------------------------

/// Basic 2-symbol vtable IR.
#[test]
fn test_emit_vtable_global_text_basic() {
    let symbols = vec![
        "landin_Clone_S_clone".to_string(),
        "landin_Clone_S_clone_from".to_string(),
    ];
    let ir = emit_vtable_global_text(".vtable.Clone.S", &symbols);
    assert_eq!(
        ir,
        "@.vtable.Clone.S = internal unnamed_addr constant \
         [2 x ptr] [ptr @landin_Clone_S_clone, ptr @landin_Clone_S_clone_from]"
    );
}

/// Empty symbols → zeroinitializer.
#[test]
fn test_emit_vtable_global_text_empty() {
    let ir = emit_vtable_global_text(".vtable.Copy.S", &[]);
    assert_eq!(
        ir,
        "@.vtable.Copy.S = internal unnamed_addr constant [0 x ptr] zeroinitializer"
    );
}

/// Single symbol.
#[test]
fn test_emit_vtable_global_text_single() {
    let symbols = vec!["landin_Drop_S_drop".to_string()];
    let ir = emit_vtable_global_text(".vtable.Drop.S", &symbols);
    assert_eq!(
        ir,
        "@.vtable.Drop.S = internal unnamed_addr constant [1 x ptr] [ptr @landin_Drop_S_drop]"
    );
}

/// Multi (3+) symbols.
#[test]
fn test_emit_vtable_global_text_multi() {
    let symbols = vec![
        "landin_S_a".to_string(),
        "landin_S_b".to_string(),
        "landin_S_c".to_string(),
    ];
    let ir = emit_vtable_global_text(".vtable.Foo.S", &symbols);
    assert!(ir.contains("[3 x ptr]"));
    assert!(ir.contains("ptr @landin_S_a, ptr @landin_S_b, ptr @landin_S_c"));
}

// ---------------------------------------------------------------------------
// "null" handling (Stage 5.43 introduced this; 5.44 bridge also handles it)
// ---------------------------------------------------------------------------

/// "null" symbol → `ptr null` literal (no `@` prefix).
#[test]
fn test_emit_vtable_global_text_null_symbol() {
    let symbols = vec!["null".to_string()];
    let ir = emit_vtable_global_text(".vtable.X.S", &symbols);
    assert_eq!(
        ir,
        "@.vtable.X.S = internal unnamed_addr constant [1 x ptr] [ptr null]"
    );
}

/// Mixed: real symbol + null.
#[test]
fn test_emit_vtable_global_text_mixed_null() {
    let symbols = vec!["landin_Clone_S_clone".to_string(), "null".to_string()];
    let ir = emit_vtable_global_text(".vtable.Clone.S", &symbols);
    assert_eq!(
        ir,
        "@.vtable.Clone.S = internal unnamed_addr constant \
         [2 x ptr] [ptr @landin_Clone_S_clone, ptr null]"
    );
}

// ---------------------------------------------------------------------------
// Format components
// ---------------------------------------------------------------------------

/// Global name appears with `@` prefix.
#[test]
fn test_emit_vtable_global_text_global_name() {
    let ir = emit_vtable_global_text(".vtable.Display.Vec", &["landin_Vec_fmt".to_string()]);
    assert!(ir.starts_with("@.vtable.Display.Vec = "));
}

/// Array type `[N x ptr]` uses correct N.
#[test]
fn test_emit_vtable_global_text_array_type() {
    let symbols = vec!["landin_S_eq".to_string(), "landin_S_ne".to_string()];
    let ir = emit_vtable_global_text(".vtable.PartialEq.S", &symbols);
    assert!(ir.contains("[2 x ptr]"));
}

/// Input `global_name` should NOT have leading `@` (function adds it).
#[test]
fn test_emit_vtable_global_text_no_leading_at_in_input() {
    // Input ".vtable.Foo.S" (no @) → output "@.vtable.Foo.S = ..."
    let ir = emit_vtable_global_text(".vtable.Foo.S", &[]);
    assert!(ir.starts_with("@.vtable.Foo.S"));
    // Verify exactly one `@` at start (no double-@)
    assert_eq!(ir.matches('@').count(), 1);
}

// ---------------------------------------------------------------------------
// **Critical**: byte-for-byte equivalence with TextEmitter::emit_vtable_global
// ---------------------------------------------------------------------------

/// Output must match `TextEmitter::emit_vtable_global()` byte-for-byte
/// on the non-null path.
#[test]
fn test_emit_vtable_global_text_match_text_emitter() {
    let global_name = ".vtable.Clone.S";
    let symbols = vec![
        "landin_Clone_S_clone".to_string(),
        "landin_Clone_S_clone_from".to_string(),
    ];

    // Get the free function output.
    let free_fn_ir = emit_vtable_global_text(global_name, &symbols);

    // Get the TextEmitter output by calling the trait method directly.
    let mut emitter = TextEmitter::new();
    let _ = emitter.emit_vtable_global(global_name, &symbols);
    let text_emitter_ir = emitter.output_with_globals();

    // The free function output should appear verbatim in the TextEmitter output.
    assert!(
        text_emitter_ir.contains(&free_fn_ir),
        "free fn IR not found in TextEmitter output.\n\
         free fn: {free_fn_ir}\n\
         TextEmitter: {text_emitter_ir}"
    );
}

/// Empty path cross-check (zeroinitializer).
#[test]
fn test_emit_vtable_global_text_match_text_emitter_empty() {
    let global_name = ".vtable.Copy.S";

    let free_fn_ir = emit_vtable_global_text(global_name, &[]);

    let mut emitter = TextEmitter::new();
    let _ = emitter.emit_vtable_global(global_name, &[]);
    let text_emitter_ir = emitter.output_with_globals();

    assert!(
        text_emitter_ir.contains(&free_fn_ir),
        "empty free fn IR not found in TextEmitter output.\n\
         free fn: {free_fn_ir}\n\
         TextEmitter: {text_emitter_ir}"
    );
}

/// Null path: the free function handles "null" → `ptr null`, but
/// `TextEmitter::emit_vtable_global()` would emit `ptr @null` (wrong).
/// This test documents the divergence — Stage 5.45 will fix TextEmitter
/// by delegating to the free function.
#[test]
fn test_emit_vtable_global_text_null_path_diverges_from_text_emitter() {
    let global_name = ".vtable.Clone.S";
    let symbols = vec!["landin_Clone_S_clone".to_string(), "null".to_string()];

    let free_fn_ir = emit_vtable_global_text(global_name, &symbols);
    // Free function emits `ptr null` (correct).
    assert!(free_fn_ir.contains("ptr null"));
    assert!(!free_fn_ir.contains("ptr @null"));

    // TextEmitter (current path) would emit `ptr @null` (incorrect, but
    // never triggered in practice because `emit_vtables()` only passes
    // real symbols). We don't assert against TextEmitter here — this test
    // documents the divergence that Stage 5.45 will resolve.
}
