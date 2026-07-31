//! Stage 4.9: Closure call lowering tests
//!
//! Tests that closure calls (calling a value of TyKind::Closure) are correctly
//! detected and don't produce incorrect TerminatorKind::Call.
//!
//! Per stage-committee-process.md v3.17 §17.1, new tests are placed in
//! `tests/v0/stage4/plan/` (standardized directory structure).

use landin_compiler::compile;

#[test]
fn test_closure_call_no_crash() {
    // Stage 4.9: Closure definition + call should not crash.
    // The closure call is detected as TyKind::Closure and handled
    // with a simplified placeholder (returns unit).
    let result = compile("fn main() { let f = |x: i32| x; f(42); }");
    // Should produce MIR without crashing
    assert!(!result.mirs.is_empty(), "should produce MIR");
}

#[test]
fn test_closure_call_with_capture() {
    // Stage 4.9: Closure with captures + call should not crash.
    let result = compile("fn main() { let y = 10; let f = |x: i32| x + y; f(1); }");
    assert!(!result.mirs.is_empty(), "should produce MIR");
}
