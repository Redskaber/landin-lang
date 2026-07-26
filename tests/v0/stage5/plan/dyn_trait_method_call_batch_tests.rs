//! Stage 5.69: emit_dyn_trait_method_calls_text_batch tests
//!
//! Tests `emit_dyn_trait_method_calls_text_batch()` — batch version of
//! emit_dyn_trait_method_call_text (Stage 5.67).
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{
    emit_dyn_trait_method_call_text, emit_dyn_trait_method_calls_text_batch,
};
use landin_compiler::mir::DynTraitMethodCall;
use landin_compiler::stdlib::StdlibTypeKind;

/// Empty input → empty Vec.
#[test]
fn test_method_calls_text_batch_empty() {
    let lines = emit_dyn_trait_method_calls_text_batch(&[]);
    assert!(lines.is_empty());
}

/// Single call → 1 IR text block.
#[test]
fn test_method_calls_text_batch_single() {
    let calls = [DynTraitMethodCall::new(
        "Drop",
        "S",
        "drop",
        0,
        0,
        StdlibTypeKind::Unit,
        vec![],
    )];
    let lines = emit_dyn_trait_method_calls_text_batch(&calls);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("; dyn Drop.S::drop"));
}

/// Multiple calls → multiple IR text blocks.
#[test]
fn test_method_calls_text_batch_multi() {
    let calls = [
        DynTraitMethodCall::new("Clone", "S", "clone", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new(
            "Clone",
            "S",
            "clone_from",
            1,
            1,
            StdlibTypeKind::Unit,
            vec![],
        ),
        DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]),
    ];
    let lines = emit_dyn_trait_method_calls_text_batch(&calls);
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("clone"));
    assert!(lines[1].contains("clone_from"));
    assert!(lines[2].contains("drop"));
}

/// Batch output == individual calls.
#[test]
fn test_method_calls_text_batch_match_individual() {
    let calls = [
        DynTraitMethodCall::new("Clone", "S", "clone", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new("Drop", "T", "drop", 0, 0, StdlibTypeKind::Unit, vec![]),
    ];
    let batch = emit_dyn_trait_method_calls_text_batch(&calls);
    let individual: Vec<String> = calls.iter().map(emit_dyn_trait_method_call_text).collect();
    assert_eq!(batch, individual);
}

/// No side effects — pure function.
#[test]
fn test_method_calls_text_batch_no_side_effects() {
    let calls = [DynTraitMethodCall::new(
        "Foo",
        "S",
        "bar",
        0,
        0,
        StdlibTypeKind::Unit,
        vec![],
    )];
    let l1 = emit_dyn_trait_method_calls_text_batch(&calls);
    let l2 = emit_dyn_trait_method_calls_text_batch(&calls);
    assert_eq!(l1, l2);
}

/// All lines contain valid LLVM IR.
#[test]
fn test_method_calls_text_batch_valid_ir() {
    let calls = [
        DynTraitMethodCall::new("Clone", "S", "clone", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new("Display", "Vec", "fmt", 0, 1, StdlibTypeKind::Unit, vec![]),
    ];
    let lines = emit_dyn_trait_method_calls_text_batch(&calls);
    for line in &lines {
        assert!(line.contains("getelementptr"));
        assert!(line.contains("load ptr"));
        assert!(line.contains("call ptr"));
    }
}

/// Real scenario: Clone (2 methods) + Drop (1 method).
#[test]
fn test_method_calls_text_batch_real_scenario() {
    let calls = [
        DynTraitMethodCall::new("Clone", "S", "clone", 0, 0, StdlibTypeKind::Unit, vec![]),
        DynTraitMethodCall::new(
            "Clone",
            "S",
            "clone_from",
            1,
            1,
            StdlibTypeKind::Unit,
            vec![],
        ),
        DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]),
    ];
    let lines = emit_dyn_trait_method_calls_text_batch(&calls);
    assert_eq!(lines.len(), 3);
}

/// Deterministic — repeated calls identical.
#[test]
fn test_method_calls_text_batch_deterministic() {
    let calls = [DynTraitMethodCall::new(
        "Foo",
        "S",
        "bar",
        0,
        0,
        StdlibTypeKind::Unit,
        vec![],
    )];
    let l1 = emit_dyn_trait_method_calls_text_batch(&calls);
    let l2 = emit_dyn_trait_method_calls_text_batch(&calls);
    assert_eq!(l1, l2);
}
