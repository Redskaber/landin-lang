//! Stage 16.31 — 通解: Borrowck on Closure MIR Bodies (Capture Mutability).
//!
//! These tests verify that borrowck correctly handles closure MIR bodies:
//! 1. Mutable captures in loops work (`|| { while x<3 { x+=1; } x }`)
//! 2. Early return inside closures works (`|| { if x>0 { return 1; } 0 }`)
//! 3. Borrowck violations inside closures are detected (soundness)
//! 4. No regressions on existing closure patterns
//!
//! Per §1.0 原則 4 "报错 > 静默": borrowck violations are now reported.
//! Per §1.0 原則 9 "正确 > 妥协": fix the root cause (mutability propagation).

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.31 test 1: Mutable capture in while loop compiles.
/// Before Stage 16.31, this was a false positive ("cannot assign twice
/// to immutable variable") because the capture extract local was
/// Immutable.
#[test]
fn stage16_31_mutable_capture_while_loop() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(
        !result.has_errors(),
        "Mutable capture in while loop should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.31 test 2: Early return inside closure compiles.
/// Before Stage 16.31, this was a false positive because the return
/// local (LocalId(0)) was Immutable.
#[test]
fn stage16_31_early_return_in_closure() {
    let result = compile("fn main() { let x=1; let f=||{ if x>0 { return 1; } 0 }; }");
    assert!(
        !result.has_errors(),
        "Early return in closure should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.31 test 3: Mutable capture with compound assignment.
#[test]
fn stage16_31_mutable_capture_compound_assign() {
    let result = compile("fn main() { let mut x=10; let f=||{ x += 5; }; }");
    assert!(
        !result.has_errors(),
        "Mutable capture compound assign should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.31 test 4: Multiple mutable captures.
#[test]
fn stage16_31_multiple_mutable_captures() {
    let result = compile("fn main() { let mut a=1; let mut b=2; let f=||{ a += b; b += 1; }; }");
    assert!(
        !result.has_errors(),
        "Multiple mutable captures should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.31 test 5: Mixed mutable and immutable captures.
#[test]
fn stage16_31_mixed_captures() {
    let result = compile("fn main() { let a=1; let mut b=2; let f=||{ b += a; }; }");
    assert!(
        !result.has_errors(),
        "Mixed captures should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.31 test 6: Soundness — use-after-move inside closure detected.
/// Borrowck on closure MIR bodies now catches violations.
#[test]
fn stage16_31_use_after_move_in_closure_detected() {
    let src = "struct R; impl Drop for R { fn drop(&mut self) {} } fn main() { let r = R; let f = || { let r2 = r; let r3 = r; }; }";
    let result = compile(src);
    // Should have borrowck errors (use-after-move of `r`).
    assert!(
        !result.errors.borrowck.is_empty(),
        "Use-after-move in closure should be detected by borrowck"
    );
}

/// Stage 16.31 test 7: No-capture closure still works (no regression).
#[test]
fn stage16_31_nocapture_no_regression() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
}

/// Stage 16.31 test 8: i32-capture closure still works (no regression).
#[test]
fn stage16_31_i32_capture_no_regression() {
    let result = compile("fn main() -> i32 { let n = 10; let f = |x| x + n; f(5) }");
    assert!(!result.has_errors());
}

/// Stage 16.31 test 9: Nested closure still works (no regression).
#[test]
fn stage16_31_nested_closure_no_regression() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || x; let _ = f()(); 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.31 test 10: Mutable capture with if-else.
#[test]
fn stage16_31_mutable_capture_if_else() {
    let result =
        compile("fn main() { let mut x=0; let f=||{ if x > 0 { x = 1; } else { x = 2; } }; }");
    assert!(
        !result.has_errors(),
        "Mutable capture if-else should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.31 test 11: Closure with mutable capture and loop.
#[test]
fn stage16_31_mutable_capture_loop() {
    let result =
        compile("fn main() { let mut x=0; let f=||{ loop { x += 1; if x >= 5 { break; } } }; }");
    assert!(
        !result.has_errors(),
        "Mutable capture loop should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.31 test 12: Closure with mutable capture and for-loop (Range).
#[test]
fn stage16_31_mutable_capture_for_loop() {
    // Note: for-loop only supports Range iterators (start..end) in v0.3.
    let result = compile("fn main() { let mut x=0; let f=||{ for i in 0..3 { x += i; } }; }");
    assert!(
        !result.has_errors(),
        "Mutable capture for-loop (Range) should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.31 test 13: Closure mutability propagation — capture from
/// a `let mut` binding is mutable in the closure body.
#[test]
fn stage16_31_capture_mutability_propagation() {
    // If mutability wasn't propagated, this would fail borrowck.
    let result = compile("fn main() -> i32 { let mut x = 0; let f = || { x = 42; }; f(); x }");
    // Note: `f()` doesn't actually mutate the outer `x` (the closure
    // captures by value, so it mutates a copy). But the closure body
    // should still compile (the extract local is mutable).
    assert!(
        !result.has_errors(),
        "Capture mutability propagation should work: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.31 test 14: Closure returning value from mutable capture.
#[test]
fn stage16_31_closure_returning_mutable_capture() {
    let result = compile("fn main() -> i32 { let mut x = 5; let f = || { x += 10; x }; f() }");
    assert!(
        !result.has_errors(),
        "Closure returning mutable capture should compile: {:?}",
        result.errors.borrowck
    );
}
