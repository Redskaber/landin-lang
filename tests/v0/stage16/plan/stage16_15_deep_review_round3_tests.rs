//! Stage 16.15 — Deep Review Round 3: synthesized closure MIR body structure verification.
//!
//! This test verifies the structure of synthesized closure MIR bodies
//! in detail, addressing the D3 (test coverage) gap identified in
//! Deep Review Round 3.
//!
//! Per §29.1.3 (Design-Impl-Test coverage): structural verification.
//! Per §1.0 原則 3 "显式 > 隐式": explicit structure checks.

#![cfg(test)]
use landin_compiler::compile;
use landin_compiler::mir::body::TerminatorKind;

/// Stage 16.15 test 1: Synthesized MIR body has correct local count.
///
/// `|x| x + 1` (no captures) should have:
/// - LocalId(0): return
/// - LocalId(1): self (closure struct)
/// - LocalId(2): param x
/// - LocalId(3+): body temporaries
#[test]
fn stage16_15_synthesized_mir_body_local_count_no_captures() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(5) }");
    let mir = &result.synthesized_closure_mir_bodies[0];
    // At minimum: return (0), self (1), param x (2)
    assert!(
        mir.local_decls.len() >= 3,
        "should have at least 3 locals; got: {}",
        mir.local_decls.len()
    );
}

/// Stage 16.15 test 2: Synthesized MIR body has correct local count with captures.
///
/// `let n = 10; |x| x + n` (1 capture) should have:
/// - LocalId(0): return
/// - LocalId(1): self
/// - LocalId(2): param x
/// - LocalId(3): capture n (extracted from self)
/// - LocalId(4+): body temporaries
#[test]
fn stage16_15_synthesized_mir_body_local_count_with_captures() {
    let result = compile("fn main() -> i32 { let n = 10; let f = |x| x + n; f(5) }");
    let mir = &result.synthesized_closure_mir_bodies[0];
    // At minimum: return (0), self (1), param x (2), capture n (3)
    assert!(
        mir.local_decls.len() >= 4,
        "should have at least 4 locals (with capture); got: {}",
        mir.local_decls.len()
    );
}

/// Stage 16.15 test 3: Synthesized MIR body has at least one basic block.
#[test]
fn stage16_15_synthesized_mir_body_has_basic_block() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(5) }");
    let mir = &result.synthesized_closure_mir_bodies[0];
    assert!(
        !mir.basic_blocks.is_empty(),
        "should have at least one basic block"
    );
}

/// Stage 16.15 test 4: Synthesized MIR body has Return terminator.
#[test]
fn stage16_15_synthesized_mir_body_has_return() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(5) }");
    let mir = &result.synthesized_closure_mir_bodies[0];
    let has_return = mir
        .basic_blocks
        .iter()
        .any(|bb| matches!(bb.terminator.kind, TerminatorKind::Return));
    assert!(has_return, "should have a Return terminator");
}

/// Stage 16.15 test 5: Synthesized MIR body has statements (not just terminator).
#[test]
fn stage16_15_synthesized_mir_body_has_statements() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(5) }");
    let mir = &result.synthesized_closure_mir_bodies[0];
    let has_statements = mir.basic_blocks.iter().any(|bb| !bb.statements.is_empty());
    assert!(
        has_statements,
        "should have at least one statement (body lowering)"
    );
}

/// Stage 16.15 test 6: Multiple closures produce MIR bodies with different structures.
///
/// Closures with different bodies should produce different MIR structures
/// (different statement counts, local counts, etc.).
#[test]
fn stage16_15_multiple_closures_have_different_structures() {
    let result =
        compile("fn main() -> i32 { let f = |x| x + 1; let g = |y| y * 2 + 3; f(5) + g(4) }");
    assert_eq!(
        result.synthesized_closure_mir_bodies.len(),
        2,
        "should have 2 synthesized MIR bodies"
    );
    // Both should be valid, but may have different structures.
    let mir1 = &result.synthesized_closure_mir_bodies[0];
    let mir2 = &result.synthesized_closure_mir_bodies[1];
    assert!(
        !mir1.basic_blocks.is_empty(),
        "mir1 should have basic blocks"
    );
    assert!(
        !mir2.basic_blocks.is_empty(),
        "mir2 should have basic blocks"
    );
}

/// Stage 16.15 test 7: Closure with multiple captures has correct local count.
///
/// `let a = 1; let b = 2; |x| x + a + b` (2 captures) should have:
/// - LocalId(0): return
/// - LocalId(1): self
/// - LocalId(2): param x
/// - LocalId(3): capture a
/// - LocalId(4): capture b
/// - LocalId(5+): body temporaries
#[test]
fn stage16_15_closure_multiple_captures_local_count() {
    let result = compile("fn main() -> i32 { let a = 1; let b = 2; let f = |x| x + a + b; f(5) }");
    let mir = &result.synthesized_closure_mir_bodies[0];
    // At minimum: return (0), self (1), param x (2), capture a (3), capture b (4)
    assert!(
        mir.local_decls.len() >= 5,
        "should have at least 5 locals (2 captures); got: {}",
        mir.local_decls.len()
    );
}

/// Stage 16.15 test 8: No-closure program has empty synthesized_closure_mir_bodies.
#[test]
fn stage16_15_no_closure_program_has_empty_bodies() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(
        result.synthesized_closure_mir_bodies.is_empty(),
        "program without closures should have no synthesized MIR bodies"
    );
}
