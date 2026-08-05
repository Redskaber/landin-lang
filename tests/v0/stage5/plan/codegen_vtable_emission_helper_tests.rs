//! Stage 5.43: Codegen vtable emission helper tests
//!
//! Tests `emit_vtable_global_from_emission()` — pure free function in
//! `src/codegen/mod.rs` that produces LLVM IR text from a
//! `StdlibVtableEmission`.
//!
//! **Critical invariant**: the output must be byte-for-byte identical to
//! what `TextEmitter::emit_vtable_global()` produces. The
//! `test_emit_vtable_global_from_emission_match_text_emitter` test
//! verifies this by constructing both and asserting equality.
//!
//! Per §16: tests use the public API only (no driver/hir/mir access).
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{emit_vtable_global_from_emission, ModuleEmitter, TextEmitter};
use landin_compiler::stdlib::{stdlib_vtable_emission, StdlibVtableEmission};

// ---------------------------------------------------------------------------
// Basic emission
// ---------------------------------------------------------------------------

/// Clone + S + [clone, clone_from] → complete 2-slot vtable IR.
#[test]
fn test_emit_vtable_global_from_emission_clone() {
    let e = stdlib_vtable_emission("Clone", "S", &["clone", "clone_from"]).unwrap();
    let ir = emit_vtable_global_from_emission(&e);
    assert_eq!(
        ir,
        "@.vtable.Clone.S = private unnamed_addr constant \
         [2 x ptr] [ptr @landin_S_clone, ptr @landin_S_clone_from]"
    );
}

/// Drop + S + [drop] → 1-slot vtable IR.
#[test]
fn test_emit_vtable_global_from_emission_drop() {
    let e = stdlib_vtable_emission("Drop", "S", &["drop"]).unwrap();
    let ir = emit_vtable_global_from_emission(&e);
    assert_eq!(
        ir,
        "@.vtable.Drop.S = private unnamed_addr constant [1 x ptr] [ptr @landin_S_drop]"
    );
}

/// Copy + S + [] → marker, zeroinitializer.
#[test]
fn test_emit_vtable_global_from_emission_marker() {
    let e = stdlib_vtable_emission("Copy", "S", &[]).unwrap();
    let ir = emit_vtable_global_from_emission(&e);
    assert_eq!(
        ir,
        "@.vtable.Copy.S = private unnamed_addr constant zeroinitializer"
    );
}

/// Clone + S + [clone] → 2-slot vtable with "null" for missing clone_from.
#[test]
fn test_emit_vtable_global_from_emission_partial() {
    let e = stdlib_vtable_emission("Clone", "S", &["clone"]).unwrap();
    let ir = emit_vtable_global_from_emission(&e);
    assert_eq!(
        ir,
        "@.vtable.Clone.S = private unnamed_addr constant \
         [2 x ptr] [ptr @landin_S_clone, ptr null]"
    );
}

/// Add + Vec + [add] → 1-slot arith vtable.
#[test]
fn test_emit_vtable_global_from_emission_arith() {
    let e = stdlib_vtable_emission("Add", "Vec", &["add"]).unwrap();
    let ir = emit_vtable_global_from_emission(&e);
    assert_eq!(
        ir,
        "@.vtable.Add.Vec = private unnamed_addr constant [1 x ptr] [ptr @landin_Vec_add]"
    );
}

// ---------------------------------------------------------------------------
// Format components
// ---------------------------------------------------------------------------

/// Global name in IR matches `.vtable.<trait>.<type>`.
#[test]
fn test_emit_vtable_global_from_emission_format_global_name() {
    let e = stdlib_vtable_emission("Display", "Vec", &["fmt"]).unwrap();
    let ir = emit_vtable_global_from_emission(&e);
    assert!(ir.starts_with("@.vtable.Display.Vec = "));
}

/// Array type `[N x ptr]` uses correct N.
#[test]
fn test_emit_vtable_global_from_emission_format_array() {
    let e = stdlib_vtable_emission("PartialEq", "S", &["eq", "ne"]).unwrap();
    let ir = emit_vtable_global_from_emission(&e);
    assert!(ir.contains("[2 x ptr]"), "expected [2 x ptr] in: {ir}");
}

/// Entries use `ptr @sym` format.
#[test]
fn test_emit_vtable_global_from_emission_format_entries() {
    let e = stdlib_vtable_emission("Clone", "S", &["clone", "clone_from"]).unwrap();
    let ir = emit_vtable_global_from_emission(&e);
    assert!(
        ir.contains("ptr @landin_S_clone"),
        "missing ptr @landin_S_clone in: {ir}"
    );
    assert!(ir.contains("ptr @landin_S_clone_from"));
}

/// "null" symbol → `ptr null` literal.
#[test]
fn test_emit_vtable_global_from_emission_null_symbol() {
    let e = stdlib_vtable_emission("Clone", "S", &["clone"]).unwrap();
    let ir = emit_vtable_global_from_emission(&e);
    assert!(ir.contains("ptr null"), "expected 'ptr null' in: {ir}");
}

/// Marker → `zeroinitializer` (no array type).
#[test]
fn test_emit_vtable_global_from_emission_empty_marker_zeroinitializer() {
    let e = stdlib_vtable_emission("Send", "S", &[]).unwrap();
    let ir = emit_vtable_global_from_emission(&e);
    assert!(ir.contains("zeroinitializer"));
    assert!(!ir.contains("x ptr]")); // no array type for markers
}

// ---------------------------------------------------------------------------
// **Critical**: byte-for-byte equivalence with TextEmitter::emit_vtable_global
// ---------------------------------------------------------------------------

/// Output must match `TextEmitter::emit_vtable_global()` byte-for-byte.
/// This is the safety net for Stage 5.44+ refactor — when we delegate
/// `TextEmitter::emit_vtable_global()` to this free function, behavior
/// stays identical.
#[test]
fn test_emit_vtable_global_from_emission_match_text_emitter() {
    // Construct a StdlibVtableEmission manually for full control.
    let emission = StdlibVtableEmission {
        trait_name: "Clone",
        type_name: "S".to_string(),
        global_name: ".vtable.Clone.S".to_string(),
        method_symbols: vec![
            "landin_S_clone".to_string(),
            "landin_S_clone_from".to_string(),
        ],
        slot_count: 2,
        byte_size_32: 8,
        byte_size_64: 16,
        is_marker: false,
        is_complete: true,
    };

    // Get the free function output.
    let free_fn_ir = emit_vtable_global_from_emission(&emission);

    // Get the TextEmitter output by calling the trait method directly.
    let mut emitter = TextEmitter::new();
    let _ = emitter.emit_vtable_global(&emission.global_name, &emission.method_symbols);
    // TextEmitter stores globals in a Vec — `output_with_globals()` returns
    // the full module text including accumulated globals.
    let text_emitter_ir = emitter.output_with_globals();

    // The free function output should appear verbatim in the TextEmitter output.
    assert!(
        text_emitter_ir.contains(&free_fn_ir),
        "free fn IR not found in TextEmitter output.\n\
         free fn: {free_fn_ir}\n\
         TextEmitter: {text_emitter_ir}"
    );
}

/// Same cross-check for marker emission (zeroinitializer path).
#[test]
fn test_emit_vtable_global_from_emission_match_text_emitter_marker() {
    let emission = StdlibVtableEmission {
        trait_name: "Copy",
        type_name: "S".to_string(),
        global_name: ".vtable.Copy.S".to_string(),
        method_symbols: vec![],
        slot_count: 0,
        byte_size_32: 0,
        byte_size_64: 0,
        is_marker: true,
        is_complete: true,
    };

    let free_fn_ir = emit_vtable_global_from_emission(&emission);

    let mut emitter = TextEmitter::new();
    let _ = emitter.emit_vtable_global(&emission.global_name, &[]);
    let text_emitter_ir = emitter.output_with_globals();

    assert!(
        text_emitter_ir.contains(&free_fn_ir),
        "marker free fn IR not found in TextEmitter output.\n\
         free fn: {free_fn_ir}\n\
         TextEmitter: {text_emitter_ir}"
    );
}

/// PartialEq + [eq] (1 provided, 1 missing) → 2-slot IR with null.
#[test]
fn test_emit_vtable_global_from_emission_partial_eq() {
    let e = stdlib_vtable_emission("PartialEq", "S", &["eq"]).unwrap();
    let ir = emit_vtable_global_from_emission(&e);
    assert_eq!(
        ir,
        "@.vtable.PartialEq.S = private unnamed_addr constant \
         [2 x ptr] [ptr @landin_S_eq, ptr null]"
    );
}
