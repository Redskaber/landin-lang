//! Stage 5.67: emit_dyn_trait_method_call_text tests
//!
//! Tests `emit_dyn_trait_method_call_text()` — converts DynTraitMethodCall
//! to LLVM IR text for a vtable indirect call.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::mir::{emit_dyn_trait_method_call_text, DynTraitMethodCall};

/// Basic call with no params.
#[test]
fn test_emit_dyn_trait_method_call_text_basic() {
    let call = DynTraitMethodCall::new("Drop", "S", "drop", 0, 0);
    let ir = emit_dyn_trait_method_call_text(&call);
    assert!(ir.contains("; dyn Drop.S::drop (slot=0, params=0)"));
    assert!(ir.contains("%vtable_ptr = getelementptr"));
    assert!(ir.contains("%method_fn = load ptr, ptr %vtable_ptr, i32 0"));
    assert!(ir.contains("%result = call ptr %method_fn(ptr %self)"));
}

/// Call with 1 param (Display::fmt).
#[test]
fn test_emit_dyn_trait_method_call_text_one_param() {
    let call = DynTraitMethodCall::new("Display", "Vec", "fmt", 0, 1);
    let ir = emit_dyn_trait_method_call_text(&call);
    assert!(ir.contains("; dyn Display.Vec::fmt (slot=0, params=1)"));
    assert!(ir.contains("ptr %self, ptr %arg0"));
}

/// Call with 2 params.
#[test]
fn test_emit_dyn_trait_method_call_text_two_params() {
    let call = DynTraitMethodCall::new("PartialEq", "S", "eq", 0, 1);
    let ir = emit_dyn_trait_method_call_text(&call);
    assert!(ir.contains("params=1"));
    assert!(ir.contains("ptr %self, ptr %arg0"));
}

/// Slot index 1 (clone_from).
#[test]
fn test_emit_dyn_trait_method_call_text_slot_1() {
    let call = DynTraitMethodCall::new("Clone", "S", "clone_from", 1, 1);
    let ir = emit_dyn_trait_method_call_text(&call);
    assert!(ir.contains("slot=1"));
    assert!(ir.contains("i32 1"));
}

/// From fat ptr constructor.
#[test]
fn test_emit_dyn_trait_method_call_text_from_fat_ptr() {
    use landin_compiler::mir::DynTraitFatPtr;
    let fp = DynTraitFatPtr::new("Clone", "MyType");
    let call = DynTraitMethodCall::from_fat_ptr(&fp, "clone", 0, 0);
    let ir = emit_dyn_trait_method_call_text(&call);
    assert!(ir.contains("; dyn Clone.MyType::clone"));
}

/// Full IR line verification.
#[test]
fn test_emit_dyn_trait_method_call_text_full_ir() {
    let call = DynTraitMethodCall::new("Display", "Vec", "fmt", 0, 1);
    let ir = emit_dyn_trait_method_call_text(&call);
    let lines: Vec<&str> = ir.lines().collect();
    assert_eq!(lines.len(), 4);
    assert!(lines[0].starts_with("; dyn"));
    assert!(lines[1].contains("getelementptr"));
    assert!(lines[2].contains("load ptr"));
    assert!(lines[3].contains("call ptr"));
}

/// No side effects — pure function.
#[test]
fn test_emit_dyn_trait_method_call_text_no_side_effects() {
    let call = DynTraitMethodCall::new("Foo", "S", "bar", 0, 0);
    let ir1 = emit_dyn_trait_method_call_text(&call);
    let ir2 = emit_dyn_trait_method_call_text(&call);
    assert_eq!(ir1, ir2);
}

/// Real scenario: Clone::clone (slot 0, 0 params).
#[test]
fn test_emit_dyn_trait_method_call_text_real_clone() {
    let call = DynTraitMethodCall::new("Clone", "S", "clone", 0, 0);
    let ir = emit_dyn_trait_method_call_text(&call);
    assert!(ir.contains("; dyn Clone.S::clone (slot=0, params=0)"));
    assert!(ir.contains("call ptr %method_fn(ptr %self)"));
}

/// Real scenario: Display::fmt (slot 0, 1 param).
#[test]
fn test_emit_dyn_trait_method_call_text_real_display() {
    let call = DynTraitMethodCall::new("Display", "Vec", "fmt", 0, 1);
    let ir = emit_dyn_trait_method_call_text(&call);
    assert!(ir.contains("; dyn Display.Vec::fmt (slot=0, params=1)"));
    assert!(ir.contains("call ptr %method_fn(ptr %self, ptr %arg0)"));
}

/// Multiple calls — independent IR.
#[test]
fn test_emit_dyn_trait_method_call_text_multiple() {
    let calls = [
        DynTraitMethodCall::new("Clone", "S", "clone", 0, 0),
        DynTraitMethodCall::new("Clone", "S", "clone_from", 1, 1),
    ];
    let irs: Vec<String> = calls.iter().map(emit_dyn_trait_method_call_text).collect();
    assert_eq!(irs.len(), 2);
    assert!(irs[0].contains("slot=0"));
    assert!(irs[1].contains("slot=1"));
}
