//! Stage 16.14 — Task 10 Step 2: Synthesized closure MIR body synthesis tests.
//!
//! These tests verify the Stage 16.14 additions:
//! 1. `build_synthesized_closure_mir_body()` builds a valid MirBody.
//! 2. `synthesized_closure_mir_bodies` field on CompileResult is populated.
//! 3. Each closure literal produces one synthesized MIR body.
//! 4. The MIR body has the correct structure (self param, closure params,
//!    capture extraction, body, return).
//!
//! Per §29.1.3 (Design-Impl-Test coverage): tests verify the MIR body
//! synthesis produces valid output.
//! Per §23: API naming compliance verified.

#![cfg(test)]
use landin_compiler::compile;
use landin_compiler::mir::body::TerminatorKind;

/// Stage 16.14 test 1: A closure literal produces a synthesized MIR body.
///
/// `let f = |x| x + 1;` should produce one entry in
/// `synthesized_closure_mir_bodies`.
#[test]
fn stage16_14_closure_literal_produces_synthesized_mir_body() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(5) }");
    assert!(
        !result.has_errors(),
        "Closure program should compile; got errors: {:?}",
        result.errors.borrowck
    );
    assert_eq!(
        result.synthesized_closure_mir_bodies.len(),
        1,
        "should have exactly 1 synthesized closure MIR body"
    );
}

/// Stage 16.14 test 2: Multiple closures produce multiple synthesized MIR bodies.
///
/// Two closure literals should produce two entries in
/// `synthesized_closure_mir_bodies`.
#[test]
fn stage16_14_multiple_closures_produce_multiple_mir_bodies() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; let g = |y| y * 2; f(5) + g(3) }");
    assert!(
        !result.has_errors(),
        "Program with multiple closures should compile; got errors: {:?}",
        result.errors.borrowck
    );
    assert_eq!(
        result.synthesized_closure_mir_bodies.len(),
        2,
        "should have exactly 2 synthesized closure MIR bodies"
    );
}

/// Stage 16.14 test 3: Synthesized MIR body has basic blocks.
///
/// Each synthesized MIR body should have at least one basic block.
#[test]
fn stage16_14_synthesized_mir_body_has_basic_blocks() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(5) }");
    let mir = &result.synthesized_closure_mir_bodies[0];
    assert!(
        !mir.basic_blocks.is_empty(),
        "synthesized MIR body should have at least one basic block"
    );
}

/// Stage 16.14 test 4: Synthesized MIR body has a Return terminator.
///
/// The last basic block should end with a Return terminator.
#[test]
fn stage16_14_synthesized_mir_body_has_return_terminator() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(5) }");
    let mir = &result.synthesized_closure_mir_bodies[0];
    // Find the last basic block with a Return terminator.
    let has_return = mir
        .basic_blocks
        .iter()
        .any(|bb| matches!(bb.terminator.kind, TerminatorKind::Return));
    assert!(
        has_return,
        "synthesized MIR body should have a Return terminator"
    );
}

/// Stage 16.14 test 5: Synthesized MIR body has local declarations.
///
/// The MIR body should have locals for: return (0), self (1), params (2+),
/// and capture extracts.
#[test]
fn stage16_14_synthesized_mir_body_has_local_declarations() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(5) }");
    let mir = &result.synthesized_closure_mir_bodies[0];
    // Should have at least: return local (0), self (1), param x (2).
    assert!(
        mir.local_decls.len() >= 3,
        "should have at least 3 locals (return, self, param); got: {}",
        mir.local_decls.len()
    );
}

/// Stage 16.14 test 6: Closure with captures produces MIR body with capture extraction.
///
/// `let n = 10; let f = |x| x + n;` — the synthesized MIR body should
/// have locals for the capture extract.
#[test]
fn stage16_14_closure_with_captures_has_capture_extraction() {
    let result = compile("fn main() -> i32 { let n = 10; let f = |x| x + n; f(5) }");
    assert!(!result.has_errors(), "program should compile");
    let mir = &result.synthesized_closure_mir_bodies[0];
    // Should have: return (0), self (1), param x (2), capture n (3).
    assert!(
        mir.local_decls.len() >= 4,
        "should have at least 4 locals (return, self, param, capture); got: {}",
        mir.local_decls.len()
    );
}

/// Stage 16.14 test 7: No closures means no synthesized MIR bodies.
///
/// A program without closures should have an empty
/// `synthesized_closure_mir_bodies`.
#[test]
fn stage16_14_no_closures_means_no_synthesized_bodies() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(
        result.synthesized_closure_mir_bodies.is_empty(),
        "program without closures should have no synthesized MIR bodies"
    );
}

/// Stage 16.14 test 8: Closure in different function produces separate MIR body.
///
/// Closures in `fn f` and `fn g` should each produce a synthesized MIR body.
#[test]
fn stage16_14_closures_in_different_functions_produce_separate_bodies() {
    let result = compile(
        "fn f() -> i32 { let c = |x| x + 1; c(5) } fn g() -> i32 { let c = |x| x + 2; c(5) } fn main() -> i32 { f() + g() }",
    );
    assert!(
        !result.has_errors(),
        "program should compile; got errors: {:?}",
        result.errors.borrowck
    );
    assert_eq!(
        result.synthesized_closure_mir_bodies.len(),
        2,
        "should have 2 synthesized MIR bodies (one per closure)"
    );
}
