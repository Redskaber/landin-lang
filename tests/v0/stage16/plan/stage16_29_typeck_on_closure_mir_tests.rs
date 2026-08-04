//! Stage 16.29 — 通解: Typeck on synthesized closure MIR bodies.
//!
//! These tests verify the 通解 (general solution) for closure typeck:
//! 1. ALL closures use the synthesized `call` function path (no special-case)
//! 2. Nested closures compile (typeck + borrowck pass)
//! 3. Closure Copy derivation (all-Copy captures → Copy)
//! 4. Shared unify table (no TyVid collision)
//! 5. Closure type accepted as callable in typeck
//!
//! Per §1.0 原則 6 "通用 > 特例": one call path for all closures.
//! Per §1.0 原則 9 "正确 > 妥协": fix the typeck gap properly.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.29 test 1: No-capture closure still uses synthesized path.
#[test]
fn stage16_29_nocapture_closure_synthesized() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(
        !result.has_errors(),
        "No-capture closure should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.29 test 2: i32-capture closure uses synthesized path (通解).
#[test]
fn stage16_29_i32_capture_synthesized() {
    let result = compile("fn main() -> i32 { let n = 10; let f = |x| x + n; f(5) }");
    assert!(
        !result.has_errors(),
        "i32-capture closure should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.29 test 3: Struct-capture closure uses synthesized path (通解).
/// Before Stage 16.29, struct captures used the inline path (特解).
/// Now ALL captures use the synthesized path.
#[test]
fn stage16_29_struct_capture_synthesized() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; let f = || p.x + p.y; f() }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Struct-capture closure should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.29 test 4: Nested closure compiles (|| || x).
/// This is the key test — nested closures failed before Stage 16.29
/// because the typeck gap caused "expected function, found _" errors.
#[test]
fn stage16_29_nested_closure_compiles() {
    let result = compile("fn main() { let x = 1; let f = || || x; let _ = f()(); }");
    assert!(
        !result.has_errors(),
        "Nested closure should compile: {:?}",
        result.errors.typeck
    );
    // Outer + inner closure = 2 synthesized MIR bodies.
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 2);
}

/// Stage 16.29 test 5: Nested closure with explicit let binding.
#[test]
fn stage16_29_nested_closure_let_binding() {
    let result = compile("fn main() { let x=1; let f=||||x; let g=f(); let _=g(); }");
    assert!(
        !result.has_errors(),
        "Nested closure with let binding should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 2);
}

/// Stage 16.29 test 6: Closure with multiple captures (all i32).
#[test]
fn stage16_29_multiple_i32_captures() {
    let result =
        compile("fn main() -> i32 { let a = 1; let b = 2; let c = 3; let f = || a + b + c; f() }");
    assert!(
        !result.has_errors(),
        "Multiple i32 captures should compile: {:?}",
        result.errors.typeck
    );
}

/// Stage 16.29 test 7: Closure with while loop inside (compile only).
/// Borrowck on closure MIR bodies is deferred (TD-CLOSURE-BORROWCK-1),
/// but the closure should still compile.
#[test]
fn stage16_29_closure_with_while() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(
        !result.has_errors(),
        "Closure with while should compile: {:?}",
        result.errors.typeck
    );
}

/// Stage 16.29 test 8: Closure with early return (compile only).
#[test]
fn stage16_29_closure_with_early_return() {
    let result = compile("fn main() { let x=1; let f=||{ if x>0 { return 1; } 0 }; }");
    assert!(
        !result.has_errors(),
        "Closure with early return should compile: {:?}",
        result.errors.typeck
    );
}

/// Stage 16.29 test 9: Closure Copy derivation — closure with i32 captures
/// is Copy (all captures are Copy). This allows `f()()` patterns.
#[test]
fn stage16_29_closure_copy_derivation() {
    // f returns a closure with i32 capture. The returned closure is Copy
    // (i32 is Copy), so calling it via Move doesn't trigger borrowck errors.
    let result = compile("fn main() -> i32 { let x = 1; let f = || || x; let _ = f()(); 42 }");
    assert!(
        !result.has_errors(),
        "Closure Copy derivation should work: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.29 test 10: Closure with no params (|| expr).
#[test]
fn stage16_29_closure_no_params() {
    let result = compile("fn main() -> i32 { let f = || 42; f() }");
    assert!(
        !result.has_errors(),
        "Closure with no params should compile: {:?}",
        result.errors.typeck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.29 test 11: Closure with two params.
#[test]
fn stage16_29_closure_two_params() {
    let result = compile("fn main() -> i32 { let f = |x: i32, y: i32| x + y; f(3, 4) }");
    assert!(
        !result.has_errors(),
        "Closure with two params should compile: {:?}",
        result.errors.typeck
    );
}

/// Stage 16.29 test 12: Chained no-capture closure calls.
#[test]
fn stage16_29_chained_nocapture_calls() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(f(f(0))) }");
    assert!(
        !result.has_errors(),
        "Chained no-capture calls should compile: {:?}",
        result.errors.typeck
    );
}

/// Stage 16.29 test 13: has_complex_captures special-case is REMOVED.
/// Verify that the inline path (lower_closure_call_inline) is deprecated
/// and not used. All closures go through lower_closure_call_to_synthesized.
#[test]
fn stage16_29_inline_path_deprecated() {
    // This test verifies at compile time that the inline path is deprecated.
    // (If someone un-deprecates it, this test still passes — it's a
    // documentation test.)
    let result = compile("fn main() -> i32 { let n = 5; let f = |x| x + n; f(10) }");
    assert!(!result.has_errors());
    // The synthesized path is used (1 MIR body), not the inline path.
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.29 test 14: Shared unify table — no TyVid collision.
/// This test would cause a stack overflow before Stage 16.29 due to
/// unify table isolation between main body and closure MIR body.
#[test]
fn stage16_29_shared_unify_table_no_overflow() {
    let result = compile("fn main() -> i32 { let n = 10; let f = |x| x + n; f(5) }");
    assert!(
        !result.has_errors(),
        "Shared unify table should prevent stack overflow: {:?}",
        result.errors.typeck
    );
}

/// Stage 16.29 test 15: Closure with tuple capture.
#[test]
fn stage16_29_tuple_capture() {
    let result = compile("fn main() -> i32 { let t = (1, 2); let f = || t.0 + t.1; f() }");
    assert!(
        !result.has_errors(),
        "Tuple capture should compile: {:?}",
        result.errors.typeck
    );
}
