//! Stage 5.64: emit_dyn_trait_fat_ptrs_text_batch tests
//!
//! Tests `emit_dyn_trait_fat_ptrs_text_batch()` — batch version of
//! emit_dyn_trait_fat_ptr_text (Stage 5.63).
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::dyn_trait_emit::{
    emit_dyn_trait_fat_ptr_text, emit_dyn_trait_fat_ptrs_text_batch,
};
use landin_compiler::mir::DynTraitFatPtr;

/// Empty input → empty Vec.
#[test]
fn test_emit_dyn_trait_fat_ptrs_text_batch_empty() {
    let lines = emit_dyn_trait_fat_ptrs_text_batch(&[]);
    assert!(lines.is_empty());
}

/// Single fat ptr → 1 line.
#[test]
fn test_emit_dyn_trait_fat_ptrs_text_batch_single() {
    let fps = [DynTraitFatPtr::new("Foo", "S")];
    let lines = emit_dyn_trait_fat_ptrs_text_batch(&fps);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("@.dynptr.Foo.S"));
}

/// Multiple fat ptrs → multiple lines.
#[test]
fn test_emit_dyn_trait_fat_ptrs_text_batch_multi() {
    let fps = [
        DynTraitFatPtr::new("Clone", "S"),
        DynTraitFatPtr::new("Drop", "S"),
        DynTraitFatPtr::new("Display", "S"),
    ];
    let lines = emit_dyn_trait_fat_ptrs_text_batch(&fps);
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("@.dynptr.Clone.S"));
    assert!(lines[1].contains("@.dynptr.Drop.S"));
    assert!(lines[2].contains("@.dynptr.Display.S"));
}

/// Batch output == individual calls.
#[test]
fn test_emit_dyn_trait_fat_ptrs_text_batch_match_individual() {
    let fps = [
        DynTraitFatPtr::new("Clone", "S"),
        DynTraitFatPtr::new("Drop", "T"),
    ];
    let batch = emit_dyn_trait_fat_ptrs_text_batch(&fps);
    let individual: Vec<String> = fps.iter().map(emit_dyn_trait_fat_ptr_text).collect();
    assert_eq!(batch, individual);
}

/// No side effects — pure function.
#[test]
fn test_emit_dyn_trait_fat_ptrs_text_batch_no_side_effects() {
    let fps = [DynTraitFatPtr::new("Foo", "S")];
    let lines1 = emit_dyn_trait_fat_ptrs_text_batch(&fps);
    let lines2 = emit_dyn_trait_fat_ptrs_text_batch(&fps);
    assert_eq!(lines1, lines2);
}

/// All lines are valid LLVM IR.
#[test]
fn test_emit_dyn_trait_fat_ptrs_text_batch_valid_ir() {
    let fps = [
        DynTraitFatPtr::new("Clone", "S"),
        DynTraitFatPtr::new("Drop", "T"),
    ];
    let lines = emit_dyn_trait_fat_ptrs_text_batch(&fps);
    for line in &lines {
        assert!(line.starts_with("@.dynptr."));
        assert!(line.contains("private unnamed_addr constant"));
        assert!(line.contains("{ ptr, ptr }"));
    }
}

/// Real scenario: S impls Clone + Drop + Display.
#[test]
fn test_emit_dyn_trait_fat_ptrs_text_batch_real_scenario() {
    let fps = [
        DynTraitFatPtr::new("Clone", "S"),
        DynTraitFatPtr::new("Drop", "S"),
        DynTraitFatPtr::new("Display", "S"),
    ];
    let lines = emit_dyn_trait_fat_ptrs_text_batch(&fps);
    assert_eq!(lines.len(), 3);
    // All share the same data symbol
    for line in &lines {
        assert!(line.contains("ptr @.data.S"));
    }
}

/// Deterministic — repeated calls identical.
#[test]
fn test_emit_dyn_trait_fat_ptrs_text_batch_deterministic() {
    let fps = [DynTraitFatPtr::new("Foo", "S")];
    let l1 = emit_dyn_trait_fat_ptrs_text_batch(&fps);
    let l2 = emit_dyn_trait_fat_ptrs_text_batch(&fps);
    assert_eq!(l1, l2);
}
