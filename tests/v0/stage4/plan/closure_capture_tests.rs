//! Stage 4.7: Closure capture analysis tests
//!
//! Tests that closure capture analysis correctly identifies external variables
//! referenced in the closure body and populates the closure's capture environment.
//!
//! Per stage-committee-process.md v3.17 §17.1, new tests are placed in
//! `tests/v0/stage4/plan/` (standardized directory structure).

use landin_compiler::compile;
use landin_compiler::mir::body::StatementKind;
use landin_compiler::mir::place::{AggregateKind, Rvalue};

/// Helper: check if any MIR body has a closure aggregate with captures.
fn has_closure_with_captures(src: &str) -> (bool, usize) {
    let result = compile(src);
    let mut found = false;
    let mut capture_count = 0;
    for mir in &result.mirs {
        for bb in &mir.basic_blocks {
            for stmt in &bb.statements {
                if let StatementKind::Assign(boxed) = &stmt.kind {
                    let (_, rvalue) = &**boxed;
                    if let Rvalue::Aggregate(AggregateKind::Closure(_, _), operands) = rvalue {
                        if !operands.is_empty() {
                            found = true;
                            capture_count = operands.len();
                        }
                    }
                }
            }
        }
    }
    (found, capture_count)
}

#[test]
fn test_closure_no_captures() {
    // Closure with no external variable references — empty capture environment.
    let (found, count) = has_closure_with_captures("fn main() { let f = |x: i32| x + 1; }");
    assert!(!found, "closure with no captures should have empty env");
    assert_eq!(count, 0);
}

#[test]
fn test_closure_captures_one_var() {
    // Closure captures one external variable `y`.
    let (found, count) =
        has_closure_with_captures("fn main() { let y = 10; let f = |x: i32| x + y; }");
    assert!(found, "closure should capture y");
    assert_eq!(count, 1, "closure should capture exactly 1 variable");
}

#[test]
fn test_closure_captures_multiple_vars() {
    // Closure captures two external variables `a` and `b`.
    let (found, count) = has_closure_with_captures(
        "fn main() { let a = 1; let b = 2; let f = |x: i32| x + a + b; }",
    );
    assert!(found, "closure should capture a and b");
    assert_eq!(count, 2, "closure should capture exactly 2 variables");
}

#[test]
fn test_closure_params_not_captured() {
    // Closure params (x) should NOT be captured — only external vars (y).
    let (found, count) =
        has_closure_with_captures("fn main() { let y = 10; let f = |x: i32| x + y; }");
    assert!(found, "closure should capture y but not x");
    assert_eq!(
        count, 1,
        "closure should capture exactly 1 variable (y), not x"
    );
}
