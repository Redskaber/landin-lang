//! Stage 5.79: codegen dyn Trait vtable indirect call tests
//!
//! Tests the FIRST codegen integration of dyn Trait data:
//! - `emit_dyn_trait_method_call()` emitter method produces LLVM IR with
//!   getelementptr + load + indirect call instructions
//! - `codegen_dyn_trait_call()` free function reads `mir.dyn_trait_calls`
//!   side-table and dispatches to the emitter
//! - `codegen_terminator`'s `TerminatorKind::Call` branch detects the
//!   `Const{ty: Error, val: Int(index)}` marker and dispatches to the
//!   dyn Trait path
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{
    codegen_dyn_trait_call_direct, AggregateEmitter, EmitType, EmitValue, TextEmitter,
};
use landin_compiler::mir::dyn_trait::DynTraitMethodCall;
use landin_compiler::mir::place::{LocalId, Operand, Place};
use landin_compiler::session::Span;
use landin_compiler::stdlib::StdlibTypeKind;
use lasso::Rodeo;

// ============================================================
// emit_dyn_trait_method_call tests (emitter-level)
// ============================================================

/// Basic: emit_dyn_trait_method_call returns a non-empty EmitValue.
#[test]
fn test_emit_dyn_trait_method_call_returns_value() {
    let mut emitter = TextEmitter::new();
    let args: Vec<(EmitType, &EmitValue)> = vec![];
    let ret = emitter.emit_dyn_trait_method_call(".dynptr.Drop.S", 0, &args, &EmitType::I32);
    assert!(!ret.is_empty());
}

/// IR contains getelementptr instruction for vtable pointer extraction.
#[test]
fn test_emit_dyn_trait_method_call_contains_gep() {
    let mut emitter = TextEmitter::new();
    let args: Vec<(EmitType, &EmitValue)> = vec![];
    emitter.emit_dyn_trait_method_call(".dynptr.Drop.S", 0, &args, &EmitType::I32);
    let output = emitter.output_with_globals();
    assert!(
        output.contains("getelementptr"),
        "expected getelementptr in IR, got: {}",
        output
    );
}

/// IR contains load instructions (vtable ptr + method fn ptr).
#[test]
fn test_emit_dyn_trait_method_call_contains_loads() {
    let mut emitter = TextEmitter::new();
    let args: Vec<(EmitType, &EmitValue)> = vec![];
    emitter.emit_dyn_trait_method_call(".dynptr.Drop.S", 0, &args, &EmitType::I32);
    let output = emitter.output_with_globals();
    // Should have at least 2 load instructions (vtable ptr + method fn ptr).
    let load_count = output.matches("load").count();
    assert!(
        load_count >= 2,
        "expected at least 2 loads, got {}: {}",
        load_count,
        output
    );
}

/// IR contains indirect call (call with %v register, not @function_name).
#[test]
fn test_emit_dyn_trait_method_call_contains_indirect_call() {
    let mut emitter = TextEmitter::new();
    let args: Vec<(EmitType, &EmitValue)> = vec![];
    emitter.emit_dyn_trait_method_call(".dynptr.Drop.S", 0, &args, &EmitType::I32);
    let output = emitter.output_with_globals();
    // Indirect call: "call i32 %v" (not "call i32 @")
    assert!(
        output.contains("call i32 %v"),
        "expected indirect call 'call i32 %v', got: {}",
        output
    );
}

/// IR references the correct dynptr symbol.
#[test]
fn test_emit_dyn_trait_method_call_references_dynptr_symbol() {
    let mut emitter = TextEmitter::new();
    let args: Vec<(EmitType, &EmitValue)> = vec![];
    emitter.emit_dyn_trait_method_call(".dynptr.Display.Vec", 0, &args, &EmitType::I32);
    let output = emitter.output_with_globals();
    assert!(
        output.contains("@.dynptr.Display.Vec"),
        "expected @.dynptr.Display.Vec in IR, got: {}",
        output
    );
}

/// IR uses the correct slot_index offset in the load instruction.
#[test]
fn test_emit_dyn_trait_method_call_uses_slot_index() {
    let mut emitter = TextEmitter::new();
    let args: Vec<(EmitType, &EmitValue)> = vec![];
    emitter.emit_dyn_trait_method_call(".dynptr.Clone.S", 1, &args, &EmitType::I32);
    let output = emitter.output_with_globals();
    // The slot_index appears in the second load (method fn ptr load).
    assert!(
        output.contains("i32 1"),
        "expected slot_index 1 in IR, got: {}",
        output
    );
}

/// Void return type: no result register assigned.
#[test]
fn test_emit_dyn_trait_method_call_void_ret_no_register() {
    let mut emitter = TextEmitter::new();
    let args: Vec<(EmitType, &EmitValue)> = vec![];
    let ret = emitter.emit_dyn_trait_method_call(".dynptr.Drop.S", 0, &args, &EmitType::Void);
    assert_eq!(ret, "0");
    let output = emitter.output_with_globals();
    assert!(
        output.contains("call void %v"),
        "expected 'call void %v' for void ret, got: {}",
        output
    );
}

/// Distinct from direct emit_call: emit_call uses @name, emit_dyn_trait_method_call uses %v.
#[test]
fn test_dyn_trait_call_distinct_from_direct_call() {
    let mut emitter = TextEmitter::new();
    let args: Vec<(EmitType, &EmitValue)> = vec![];
    // Direct call: uses @function_name
    emitter.emit_call("direct_fn", &args, &EmitType::I32);
    // Dyn Trait call: uses %v register
    emitter.emit_dyn_trait_method_call(".dynptr.Drop.S", 0, &args, &EmitType::I32);
    let output = emitter.output_with_globals();
    assert!(output.contains("call i32 @direct_fn"));
    assert!(output.contains("call i32 %v"));
}

// ============================================================
// codegen_dyn_trait_call tests (free function level)
// ============================================================

/// Helper: build a DynTraitMethodCall for Drop::S::drop.
fn make_call_info() -> DynTraitMethodCall {
    DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![])
}

/// codegen_dyn_trait_call_direct returns a non-empty EmitValue.
#[test]
fn test_codegen_dyn_trait_call_returns_value() {
    let call_info = make_call_info();
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![Operand::Copy(Place::local(LocalId(0), Span::DUMMY))];

    let ret = codegen_dyn_trait_call_direct(
        &mut emitter,
        &call_info,
        &args,
        &interner,
        &layouts,
        None,
        &std::collections::HashMap::new(),
    );
    assert!(!ret.is_empty());
}

/// codegen_dyn_trait_call_direct produces IR with vtable indirect call.
#[test]
fn test_codegen_dyn_trait_call_produces_vtable_ir() {
    let call_info = make_call_info();
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![Operand::Copy(Place::local(LocalId(0), Span::DUMMY))];

    codegen_dyn_trait_call_direct(
        &mut emitter,
        &call_info,
        &args,
        &interner,
        &layouts,
        None,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    assert!(output.contains("@.dynptr.Drop.S"));
    assert!(output.contains("getelementptr"));
    assert!(output.contains("load"));
    assert!(output.contains("call void %v"));
}

/// codegen_dyn_trait_call_direct uses correct dynptr symbol for trait/type.
#[test]
fn test_codegen_dyn_trait_call_uses_correct_dynptr_symbol() {
    let call_info =
        DynTraitMethodCall::new("Display", "Vec", "fmt", 2, 1, StdlibTypeKind::Unit, vec![]);
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![Operand::Copy(Place::local(LocalId(0), Span::DUMMY))];

    codegen_dyn_trait_call_direct(
        &mut emitter,
        &call_info,
        &args,
        &interner,
        &layouts,
        None,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    assert!(output.contains("@.dynptr.Display.Vec"));
    assert!(output.contains("i32 2")); // slot_index
}

// Stage 15.65: The OOB panic test was removed — the legacy side-table
// index lookup is gone. codegen_dyn_trait_call_direct takes the call info
// directly, so there's no index to be out of bounds.

// ============================================================
// codegen_terminator integration: TerminatorKind::Call dispatch
// ============================================================

/// Stage 15.65: codegen_dyn_trait_call_direct takes the call info directly.
/// The legacy marker Const (`Error + Int(index)`) is no longer used.
#[test]
fn test_codegen_terminator_dyn_trait_dispatch_via_direct() {
    // The new API takes the call info directly — no marker Const needed.
    let call_info = make_call_info();
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![Operand::Copy(Place::local(LocalId(0), Span::DUMMY))];

    let ret = codegen_dyn_trait_call_direct(
        &mut emitter,
        &call_info,
        &args,
        &interner,
        &layouts,
        None,
        &std::collections::HashMap::new(),
    );
    assert!(!ret.is_empty(), "expected non-empty EmitValue");
}

/// Multiple dyn Trait calls: codegen_dyn_trait_call_direct handles distinct call info.
#[test]
fn test_codegen_dyn_trait_call_multiple_distinct() {
    let call1 = DynTraitMethodCall::new("Drop", "A", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    let call2 = DynTraitMethodCall::new("Drop", "B", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);

    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![Operand::Copy(Place::local(LocalId(0), Span::DUMMY))];

    // First call → Drop.A
    codegen_dyn_trait_call_direct(
        &mut emitter,
        &call1,
        &args,
        &interner,
        &layouts,
        None,
        &std::collections::HashMap::new(),
    );
    // Second call → Drop.B
    codegen_dyn_trait_call_direct(
        &mut emitter,
        &call2,
        &args,
        &interner,
        &layouts,
        None,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    assert!(output.contains("@.dynptr.Drop.A"));
    assert!(output.contains("@.dynptr.Drop.B"));
}

/// IR for dyn Trait call is well-formed: gep + 2 loads + call.
///
/// Stage 90 (v0.8 — TD-DYN-TRAIT-DATA-PTR-EXTRACT): When args is non-empty,
/// the data pointer is extracted from the fat pointer (1 extra GEP + 1 extra
/// load). With 0 args (this test), the data pointer extraction is emitted
/// but the data pointer is not passed to the call (no args to pass it to).
/// So the counts are: 1 GEP (vtable) + 1 GEP (data) = 2 GEPs, 2 loads
/// (vtable + method) + 1 load (data) = 3 loads, 1 call.
#[test]
fn test_dyn_trait_call_ir_well_formed() {
    let mut emitter = TextEmitter::new();
    let args: Vec<(EmitType, &EmitValue)> = vec![];
    emitter.emit_dyn_trait_method_call(".dynptr.Drop.S", 0, &args, &EmitType::I32);
    let output = emitter.output_with_globals();

    let gep_count = output.matches("getelementptr").count();
    let load_count = output.matches("load").count();
    let call_count = output.matches("call i32 %v").count();

    // Stage 90: 2 GEPs (vtable field + data field), 3 loads (vtable + method
    // + data), 1 call. The data GEP + load is emitted unconditionally for
    // future use (the receiver arg will use it when args is non-empty).
    assert_eq!(
        gep_count, 2,
        "expected 2 getelementptr (vtable + data), got {}",
        gep_count
    );
    assert_eq!(
        load_count, 3,
        "expected 3 loads (vtable + method + data), got {}",
        load_count
    );
    assert_eq!(
        call_count, 1,
        "expected 1 indirect call, got {}",
        call_count
    );
}
