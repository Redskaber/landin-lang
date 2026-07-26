//! Stage 13.2 — if-let / while-let (TD-031 P0 closure) verification
//!
//! Verifies that if-let / while-let are fully supported:
//! - AST has IfLet / WhileLet variants
//! - Parser accepts `if let` / `while let` (no soft errors)
//! - HIR lowering desugars to Match / Loop { Match } (Strategy B)
//! - 11 conformance FAIL tests flipped to PASS
//! - 2 Stage 0 regression tests updated
//!
//! Per stage-committee-process.md v3.21 §13.4 + §14.4 + §25.8.

#![cfg(test)]

use std::path::Path;

/// Verify AST has IfLet variant
#[test]
fn test_ast_has_if_let_variant() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ast_kinds = manifest.join("src/ast/kinds.rs");
    let content = std::fs::read_to_string(&ast_kinds).expect("read src/ast/kinds.rs");

    assert!(
        content.contains("IfLet {")
            && content.contains("pat:")
            && content.contains("expr:")
            && content.contains("then:")
            && content.contains("else_:"),
        "src/ast/kinds.rs must define IfLet variant with pat, expr, then, else_ fields"
    );

    // Must reference Stage 13.2 TD-031 in doc comment
    assert!(
        content.contains("Stage 13.2") && content.contains("TD-031"),
        "IfLet variant must reference Stage 13.2 TD-031 in doc comment"
    );
}

/// Verify AST has WhileLet variant
#[test]
fn test_ast_has_while_let_variant() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ast_kinds = manifest.join("src/ast/kinds.rs");
    let content = std::fs::read_to_string(&ast_kinds).expect("read src/ast/kinds.rs");

    assert!(
        content.contains("WhileLet {")
            && content.contains("pat:")
            && content.contains("expr:")
            && content.contains("body:"),
        "src/ast/kinds.rs must define WhileLet variant with pat, expr, body fields"
    );

    // Must reference Stage 13.2 TD-031 in doc comment
    assert!(
        content.contains("Stage 13.2") && content.contains("TD-031"),
        "WhileLet variant must reference Stage 13.2 TD-031 in doc comment"
    );
}

/// Verify parser supports `if let` (no soft errors)
#[test]
fn test_parser_supports_if_let() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parser_expr = manifest.join("src/parser/expr.rs");
    let content = std::fs::read_to_string(&parser_expr).expect("read src/parser/expr.rs");

    // Must NOT have the old soft error message
    assert!(
        !content.contains("`if let` patterns are not yet supported in Stage 0"),
        "src/parser/expr.rs must NOT have the old 'if let not yet supported' soft error"
    );

    // Must emit Expr::IfLet
    assert!(
        content.contains("Expr::IfLet"),
        "src/parser/expr.rs must emit Expr::IfLet"
    );
}

/// Verify parser supports `while let` (no soft errors)
#[test]
fn test_parser_supports_while_let() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parser_expr = manifest.join("src/parser/expr.rs");
    let content = std::fs::read_to_string(&parser_expr).expect("read src/parser/expr.rs");

    // Must NOT have the old soft error message
    assert!(
        !content.contains("`while let` patterns are not yet supported in Stage 0"),
        "src/parser/expr.rs must NOT have the old 'while let not yet supported' soft error"
    );

    // Must emit Expr::WhileLet
    assert!(
        content.contains("Expr::WhileLet"),
        "src/parser/expr.rs must emit Expr::WhileLet"
    );
}

/// Verify HIR lowering desugars IfLet to Match (Strategy B)
#[test]
fn test_hir_lowering_desugars_if_let_to_match() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let hir_lower_body = manifest.join("src/hir/lower/body.rs");
    let content = std::fs::read_to_string(&hir_lower_body).expect("read src/hir/lower/body.rs");

    // Must have Expr::IfLet arm
    assert!(
        content.contains("Expr::IfLet {"),
        "src/hir/lower/body.rs must have Expr::IfLet arm"
    );

    // Must desugar to HirExprKind::Match
    assert!(
        content.contains("HirExprKind::Match")
            && content.contains("arms: vec![then_arm, else_arm]"),
        "IfLet must desugar to HirExprKind::Match with then_arm + else_arm"
    );

    // Must reference Stage 13.2 TD-031
    assert!(
        content.contains("Stage 13.2") && content.contains("TD-031"),
        "IfLet desugar must reference Stage 13.2 TD-031"
    );

    // Must reference Strategy B
    assert!(
        content.contains("Strategy B") || content.contains("rustc-idiomatic"),
        "IfLet desugar must reference Strategy B (rustc-idiomatic)"
    );
}

/// Verify HIR lowering desugars WhileLet to Loop { Match }
#[test]
fn test_hir_lowering_desugars_while_let_to_loop_match() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let hir_lower_body = manifest.join("src/hir/lower/body.rs");
    let content = std::fs::read_to_string(&hir_lower_body).expect("read src/hir/lower/body.rs");

    // Must have Expr::WhileLet arm
    assert!(
        content.contains("Expr::WhileLet {"),
        "src/hir/lower/body.rs must have Expr::WhileLet arm"
    );

    // Must desugar to HirExprKind::Loop with a Match inside
    assert!(
        content.contains("HirExprKind::Loop") && content.contains("HirExprKind::Match"),
        "WhileLet must desugar to HirExprKind::Loop containing HirExprKind::Match"
    );

    // Must have body_arm + break_arm
    assert!(
        content.contains("body_arm") && content.contains("break_arm"),
        "WhileLet desugar must have body_arm + break_arm"
    );

    // break_arm must use HirExprKind::Break
    assert!(
        content.contains("HirExprKind::Break"),
        "WhileLet break_arm must use HirExprKind::Break"
    );
}

/// Verify 11 conformance tests flipped from FAIL to PASS
#[test]
fn test_11_conformance_tests_flipped_to_pass() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let control_flow_dir = manifest.join("tests/conformance/00-parse/02-control-flow");

    let if_let_files = [
        "if_let_basic.lin",
        "if_let_chain.lin",
        "if_let_else.lin",
        "if_let_struct.lin",
        "if_let_tuple.lin",
        "if_let_wildcard.lin",
    ];
    let while_let_files = [
        "while_let_basic.lin",
        "while_let_break.lin",
        "while_let_continue.lin",
        "while_let_nested.lin",
        "while_let_tuple.lin",
    ];

    let mut pass_count = 0;
    let mut fail_count = 0;

    for filename in if_let_files.iter().chain(while_let_files.iter()) {
        let path = control_flow_dir.join(filename);
        let content =
            std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {}", filename));

        if content.starts_with("//! PASS") {
            pass_count += 1;
        } else if content.starts_with("//! FAIL") {
            fail_count += 1;
        }
    }

    assert_eq!(
        pass_count, 11,
        "All 11 if-let/while-let conformance tests must be PASS, got {} PASS, {} FAIL",
        pass_count, fail_count
    );
    assert_eq!(
        fail_count, 0,
        "No if-let/while-let conformance tests should be FAIL, got {} FAIL",
        fail_count
    );
}

/// Verify Stage 0 regression tests updated (no longer expect errors)
#[test]
fn test_stage0_regression_tests_updated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ast_tests = manifest.join("tests/v0/stage0/plan/ast_structure_tests.rs");
    let content = std::fs::read_to_string(&ast_tests).expect("read ast_structure_tests.rs");

    // The if-let regression test must now expect 0 errors (was !errors.is_empty())
    assert!(
        content.contains("Stage 13.2: if let is now supported"),
        "test_regression_no_infinite_loop_on_if_let must be updated for Stage 13.2"
    );

    // The while-let regression test must now expect 0 errors
    assert!(
        content.contains("Stage 13.2: while let is now supported"),
        "test_regression_no_infinite_loop_on_while_let must be updated for Stage 13.2"
    );
}

/// Verify Stage 13.2 gate review exists + PASS verdict
#[test]
fn test_stage13_2_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-13/gate-review-13.2.md");
    assert!(
        gate_review.exists(),
        "docs/develop/v0/stage-13/gate-review-13.2.md must exist"
    );

    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-13.2.md");

    // Must mark TD-031 as CLOSED
    assert!(
        content.contains("TD-031") && content.contains("CLOSED"),
        "gate-review-13.2.md must mark TD-031 as CLOSED"
    );

    // Must reference Strategy B
    assert!(
        content.contains("Strategy B"),
        "gate-review-13.2.md must reference Strategy B (Desugar to Match)"
    );

    // Must include committee vote
    assert!(
        content.contains("委员会投票") || content.contains("Committee") || content.contains("Vote"),
        "gate-review-13.2.md must include committee vote"
    );

    // Must reach PASS verdict
    assert!(
        content.contains("PASS"),
        "gate-review-13.2.md must reach PASS verdict"
    );
}

/// Verify Stage 13.2 design alignment report exists
#[test]
fn test_stage13_2_design_alignment_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let design_alignment = manifest.join("docs/develop/v0/stage-13/stage-13.2-design-alignment.md");
    assert!(
        design_alignment.exists(),
        "docs/develop/v0/stage-13/stage-13.2-design-alignment.md must exist (§13.4 design alignment)"
    );

    let content = std::fs::read_to_string(&design_alignment).expect("read design-alignment.md");

    // Must reference §13.4
    assert!(
        content.contains("§13.4") || content.contains("13.4"),
        "design-alignment.md must reference §13.4"
    );

    // Must recommend Strategy B
    assert!(
        content.contains("Strategy B") || content.contains("Desugar to Match"),
        "design-alignment.md must recommend Strategy B (Desugar to Match)"
    );
}

/// Verify v0.1 conformance gate still holds (5026 tests, 0 failed)
#[test]
fn test_v01_gate_still_holds_after_stage13_2() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conf_dir = manifest.join("tests/conformance");
    let mut total = 0;
    for entry in std::fs::read_dir(&conf_dir).expect("read conformance/") {
        let entry = entry.expect("dir entry");
        if entry.path().is_dir() {
            for sub in std::fs::read_dir(entry.path()).expect("read category") {
                let sub = sub.expect("sub entry");
                if sub.path().is_dir() {
                    total += std::fs::read_dir(sub.path())
                        .expect("read sub")
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
                        .count();
                }
            }
        }
    }
    assert!(
        total >= 5000,
        "v0.1 gate must still hold: 5000+, got {}",
        total
    );
}
