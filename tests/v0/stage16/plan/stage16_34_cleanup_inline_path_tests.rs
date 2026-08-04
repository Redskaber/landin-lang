//! Stage 16.34 — Task 10 Step 5: Clean up deprecated inline closure path.
//!
//! These tests verify that the cleanup (removing `closure_bodies` side-table
//! and `lower_closure_call_inline`) doesn't break any closure patterns:
//! 1. All closure patterns still compile (type-based check works)
//! 2. Let-bound closures work (type propagation works)
//! 3. Re-let-bound closures work (`let h = g;` where g is a closure)
//! 4. No regressions on any closure feature
//!
//! Per §1.0 原則 5 "去除兼容思维": dead code removed, no behavior change.
//! Per §23 rule 5 (DRY): type is the single source of truth.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.34 test 1: No-capture closure still works (type-based check).
#[test]
fn stage16_34_nocapture_closure_type_check() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors(), "{:?}", result.errors);
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.34 test 2: i32-capture closure still works.
#[test]
fn stage16_34_i32_capture_type_check() {
    let result = compile("fn main() -> i32 { let n = 10; let f = |x| x + n; f(5) }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.34 test 3: Let-bound closure works (type propagation).
#[test]
fn stage16_34_let_bound_closure() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; let g = f; g(10) }");
    assert!(!result.has_errors(), "{:?}", result.errors);
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.34 test 4: Re-let-bound closure works.
#[test]
fn stage16_34_re_let_bound_closure() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; let g = f; let h = g; h(10) }");
    assert!(!result.has_errors(), "{:?}", result.errors);
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.34 test 5: Struct-capture closure still works.
#[test]
fn stage16_34_struct_capture_type_check() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; let f = || p.x + p.y; f() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.34 test 6: Mutable capture closure still works.
#[test]
fn stage16_34_mutable_capture_type_check() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(!result.has_errors());
}

/// Stage 16.34 test 7: Nested closure still works.
#[test]
fn stage16_34_nested_closure_type_check() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || x; let _ = f()(); 42 }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 2);
}

/// Stage 16.34 test 8: Triple-nested closure still works.
#[test]
fn stage16_34_triple_nested_type_check() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 3);
}

/// Stage 16.34 test 9: Let-bound nested closure works.
#[test]
fn stage16_34_let_bound_nested_closure() {
    let result =
        compile("fn main() -> i32 { let x = 1; let f = || || x; let g = f; let _ = g()(); 42 }");
    assert!(!result.has_errors(), "{:?}", result.errors);
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 2);
}

/// Stage 16.34 test 10: Closure with two params still works.
#[test]
fn stage16_34_two_params_type_check() {
    let result = compile("fn main() -> i32 { let f = |x: i32, y: i32| x + y; f(3, 4) }");
    assert!(!result.has_errors());
}

/// Stage 16.34 test 11: Chained no-capture calls still work.
#[test]
fn stage16_34_chained_calls_type_check() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(f(f(0))) }");
    assert!(!result.has_errors());
}

/// Stage 16.34 test 12: Closure returning closure with param.
#[test]
fn stage16_34_closure_returning_closure_with_param() {
    let result =
        compile("fn main() -> i32 { let x = 10; let f = || |y| x + y; let g = f(); g(5) }");
    assert!(!result.has_errors());
}
