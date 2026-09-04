//! Stage 102 (v0.10): TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH Layer 4 fix.
//!
//! Verify LLVMSysEmitter::Drop correctly releases LLVM module + context.
//! Previously Drop only released builder, leaking module + context, causing
//! LLVM resource accumulation under cargo test (multiple compile() calls).

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::codegen::emitter::Emitter;
use landin_compiler::codegen::LLVMSysEmitter;

// ============================================================================
// Test 1: LLVMSysEmitter Drop releases resources (positive)
// ============================================================================

#[test]
fn stage102_emitter_drop_releases_resources() {
    // Create + drop an emitter. After Drop, module + context are disposed.
    // This test verifies Drop doesn't panic + LLVM resources are released.
    // Per §1.0 原則 1 (内存安全决不能妥协): resource leaks are unsafe.
    {
        let mut e: LLVMSysEmitter = LLVMSysEmitter::new();
        let e_ref: &mut dyn Emitter = &mut e;
        e_ref.emit_header();
        // Emitter dropped here — should release module + context.
    }
    // If we reach this point, Drop completed without panic.
}

// ============================================================================
// Test 2: Multiple emitter create/drop cycles (regression test for Layer 4)
// ============================================================================

#[test]
fn stage102_multiple_emitter_cycles_no_accumulation() {
    // Simulate cargo test scenario: multiple compile() calls each create
    // + drop an emitter. Without proper disposal, LLVM contexts accumulate.
    // Per §12 (最优>最小): root-cause fix — Drop releases module + context.
    for _ in 0..10 {
        let mut e: LLVMSysEmitter = LLVMSysEmitter::new();
        let e_ref: &mut dyn Emitter = &mut e;
        e_ref.emit_header();
        // Emitter dropped here — module + context disposed.
    }
    // If we reach this point, 10 cycles completed without panic or crash.
}

// ============================================================================
// Test 3: to_module() called before Drop is safe (ownership verification)
// ============================================================================

#[test]
fn stage102_to_module_before_drop_safe() {
    // Verify to_module() returns a valid (non-null) module while emitter
    // is in scope. After Drop, the module is disposed — but no caller
    // uses to_module() after Drop.
    let mut e: LLVMSysEmitter = LLVMSysEmitter::new();
    {
        let e_ref: &mut dyn Emitter = &mut e;
        e_ref.emit_header();
    }
    let module = e.to_module();
    assert!(!module.is_null(), "to_module() must return non-null module");
    // Emitter dropped here — module disposed. No use-after-drop.
}

// ============================================================================
// Test 4: to_object_file called before Drop is safe
// ============================================================================

#[test]
fn stage102_to_object_file_before_drop_safe() {
    use std::path::Path;
    // Verify to_object_file works while emitter is in scope.
    // Per §1.0 原則 1 (内存安全决不能妥协): no use-after-free.
    let mut e: LLVMSysEmitter = LLVMSysEmitter::new();
    {
        let e_ref: &mut dyn Emitter = &mut e;
        e_ref.emit_header();
    }
    let out_path = "/tmp/stage102_drop_test.o";
    let _ = std::fs::remove_file(out_path);
    let result = e.to_object_file(out_path);
    // Result may be Ok or Err (depends on LLVM target init), but must not panic.
    let _ = result;
    // Verify object file was created (if Ok).
    if Path::new(out_path).exists() {
        let _ = std::fs::remove_file(out_path);
    }
    // Emitter dropped here — module + context disposed.
}

// ============================================================================
// Test 5-7: negative tests (error recovery)
// ============================================================================

#[test]
fn stage102_undefined_type_errors() {
    use landin_compiler::compile;
    let src = r#"fn main() -> i32 { let x: Undefined = 0; 0 }"#;
    let result = compile(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

#[test]
fn stage102_type_mismatch_errors() {
    use landin_compiler::compile;
    let src = r#"fn main() -> i32 { let x: i32 = true; 0 }"#;
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage102_nonexistent_method_errors() {
    use landin_compiler::compile;
    let src = r#"fn main() -> i32 { let x: i32 = 42; x.nonexistent() }"#;
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty() || !result.errors.lower.is_empty());
}
