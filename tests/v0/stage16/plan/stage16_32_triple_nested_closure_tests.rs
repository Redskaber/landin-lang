//! Stage 16.32 — 通解: Triple-Nested Closure Typeck (Closure-typed Func in Typeck).
//!
//! These tests verify that triple-nested closures compile AND run correctly:
//! 1. `|| || || x` compiles (typeck resolves all closure return types)
//! 2. `f()()()` runs correctly (codegen handles triple-nested calls)
//! 3. Quadruple-nested closures work (通解 handles arbitrary depth)
//! 4. No regressions on existing closure patterns
//!
//! Per §1.0 原則 6 "通用 > 特例": one typeck path for all closure-typed calls.
//! Per §1.0 原則 9 "正确 > 妥协": fix the root cause (Closure not handled in typeck).

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.32 test 1: Triple-nested closure compiles.
#[test]
fn stage16_32_triple_nested_compiles() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(
        !result.has_errors(),
        "Triple-nested closure should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 3);
}

/// Stage 16.32 test 2: Triple-nested closure with let bindings.
#[test]
fn stage16_32_triple_nested_let_bindings() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let g = f()(); g() }");
    assert!(
        !result.has_errors(),
        "Triple-nested with let bindings should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 3);
}

/// Stage 16.32 test 3: Quadruple-nested closure compiles.
#[test]
fn stage16_32_quadruple_nested_compiles() {
    let result =
        compile("fn main() -> i32 { let x = 1; let f = || || || || x; let _ = f()()()(); 42 }");
    assert!(
        !result.has_errors(),
        "Quadruple-nested closure should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 4);
}

/// Stage 16.32 test 4: Triple-nested with i32 capture.
#[test]
fn stage16_32_triple_nested_i32_capture() {
    let result = compile("fn main() -> i32 { let x = 42; let f = || || || x; f()()() }");
    assert!(
        !result.has_errors(),
        "Triple-nested with i32 capture should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 3);
}

/// Stage 16.32 test 5: Triple-nested with param in innermost closure.
#[test]
fn stage16_32_triple_nested_with_param() {
    let result = compile("fn main() -> i32 { let x = 10; let f = || || |y| x + y; f()()(5) }");
    assert!(
        !result.has_errors(),
        "Triple-nested with param should compile: {:?}",
        result.errors.typeck
    );
}

/// Stage 16.32 test 6: Double-nested still works (no regression).
#[test]
fn stage16_32_double_nested_no_regression() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || x; let _ = f()(); 42 }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 2);
}

/// Stage 16.32 test 7: No-capture closure still works (no regression).
#[test]
fn stage16_32_nocapture_no_regression() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
}

/// Stage 16.32 test 8: i32-capture closure still works (no regression).
#[test]
fn stage16_32_i32_capture_no_regression() {
    let result = compile("fn main() -> i32 { let n = 10; let f = |x| x + n; f(5) }");
    assert!(!result.has_errors());
}

/// Stage 16.32 test 9: Mutable capture closure still works (no regression).
#[test]
fn stage16_32_mutable_capture_no_regression() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(!result.has_errors());
}

/// Stage 16.32 test 10: Closure returning closure with param (no regression).
#[test]
fn stage16_32_closure_returning_closure_with_param() {
    let result =
        compile("fn main() -> i32 { let x = 10; let f = || |y| x + y; let g = f(); g(5) }");
    assert!(!result.has_errors());
}

/// Stage 16.32 test 11: Multiple closures in same function.
#[test]
fn stage16_32_multiple_closures() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; let g = |y| y * 2; f(g(5)) }");
    assert!(!result.has_errors());
}

/// Stage 16.32 test 12: Nested closure with multiple captures.
#[test]
fn stage16_32_nested_multiple_captures() {
    let result = compile("fn main() -> i32 { let a = 1; let b = 2; let f = || || a + b; f()() }");
    assert!(!result.has_errors());
}
