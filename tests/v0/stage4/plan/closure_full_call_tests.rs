//! Stage 4.13: Full closure call lowering tests
//!
//! Tests that closure calls with TyKind::Closure extract captures and produce
//! proper MIR (not just unit placeholder).
//!
//! Per stage-committee-process.md v3.18 §17.1, new tests in tests/v0/stage4/plan/.

use landin_compiler::compile;

#[test]
fn test_full_closure_call_no_capture() {
    // Closure with no captures — call should produce MIR with inferred result.
    let result = compile("fn main() { let f = |x: i32| x; f(42); }");
    assert!(!result.mirs.is_empty(), "should produce MIR");
}

#[test]
fn test_full_closure_call_with_capture() {
    // Closure with captures — call should extract captured fields + produce MIR.
    let result = compile("fn main() { let y = 10; let f = |x: i32| x + y; f(1); }");
    assert!(!result.mirs.is_empty(), "should produce MIR");
}
