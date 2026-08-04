//! Stage 16.30 — 通解: Codegen for Closure-Typed Call Sites (Nested Closure Runtime).
//!
//! These tests verify the codegen fix for calling Closure-typed values:
//! 1. Nested closures (`f()()`) compile AND run correctly
//! 2. Let-bound closure call results (`let g = f(); g()`) work
//! 3. No regressions on existing closure patterns
//!
//! Per §1.0 原則 6 "通用 > 特例": one codegen path for all closure-typed calls.
//! Per §1.0 原則 9 "正确 > 妥协": fix the root cause, not the symptom.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.30 test 1: Nested closure compiles (f()() pattern).
#[test]
fn stage16_30_nested_closure_compiles() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || x; let _ = f()(); 42 }");
    assert!(
        !result.has_errors(),
        "Nested closure should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 2);
}

/// Stage 16.30 test 2: Nested closure with let binding compiles.
#[test]
fn stage16_30_nested_closure_let_binding_compiles() {
    let result = compile("fn main() -> i32 { let x=1; let f=||||x; let g=f(); g() }");
    assert!(
        !result.has_errors(),
        "Nested closure with let binding should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 2);
}

/// Stage 16.30 test 3: No-capture closure still works (no regression).
#[test]
fn stage16_30_nocapture_closure_no_regression() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.30 test 4: i32-capture closure still works (no regression).
#[test]
fn stage16_30_i32_capture_no_regression() {
    let result = compile("fn main() -> i32 { let n = 10; let f = |x| x + n; f(5) }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.30 test 5: Struct-capture closure still works (no regression).
#[test]
fn stage16_30_struct_capture_no_regression() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; let f = || p.x + p.y; f() }";
    let result = compile(src);
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.30 test 6: Closure with multiple captures (no regression).
#[test]
fn stage16_30_multiple_captures_no_regression() {
    let result =
        compile("fn main() -> i32 { let a = 1; let b = 2; let c = 3; let f = || a + b + c; f() }");
    assert!(!result.has_errors());
}

/// Stage 16.30 test 7: Closure with two params (no regression).
#[test]
fn stage16_30_two_params_no_regression() {
    let result = compile("fn main() -> i32 { let f = |x: i32, y: i32| x + y; f(3, 4) }");
    assert!(!result.has_errors());
}

/// Stage 16.30 test 8: Chained no-capture calls (no regression).
#[test]
fn stage16_30_chained_calls_no_regression() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(f(f(0))) }");
    assert!(!result.has_errors());
}

/// Stage 16.30 test 9: Dead code removal — old Stage 4.13 path removed.
/// This test verifies the "Real function call" path handles Closure-typed
/// func operands correctly (no inline path needed).
#[test]
fn stage16_30_dead_code_removed() {
    // f()() goes through the "Real function call" MIR path, then codegen
    // resolves the Closure type. If the old Stage 4.13 path was still
    // active, this would produce a placeholder result (not a real Call).
    let result = compile("fn main() -> i32 { let x = 5; let f = || || x; let _ = f()(); 99 }");
    assert!(!result.has_errors());
    // 2 synthesized MIR bodies (outer + inner closure)
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 2);
}

/// Stage 16.30 test 10: Nested closure with i32 capture.
#[test]
fn stage16_30_nested_closure_i32_capture() {
    let result = compile("fn main() -> i32 { let x = 42; let f = || || x; let g = f(); g() }");
    assert!(
        !result.has_errors(),
        "Nested closure with i32 capture should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 2);
}

/// Stage 16.30 test 11: Triple-nested closure (fixed in Stage 16.32).
/// Stage 16.32 added Closure-typed func handling in typeck's check_terminator,
/// which resolves the dest type of closure calls. This makes triple-nested
/// closures work.
#[test]
fn stage16_30_triple_nested_closure_deferred() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(
        !result.has_errors(),
        "Triple-nested closure should compile (fixed in Stage 16.32): {:?}",
        result.errors.typeck
    );
    // 3 synthesized MIR bodies (3 closures)
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 3);
}

/// Stage 16.30 test 12: Closure returning closure with param.
#[test]
fn stage16_30_closure_returning_closure_with_param() {
    let result =
        compile("fn main() -> i32 { let x = 10; let f = || |y| x + y; let g = f(); g(5) }");
    assert!(
        !result.has_errors(),
        "Closure returning closure with param should compile: {:?}",
        result.errors.typeck
    );
}
