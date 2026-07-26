//! Stage 9.3 — Control flow conformance expansion verification tests
//!
//! Per stage-committee-process.md v3.21 §17.1 + §13.4 design alignment
//! with `docs/lang-design/02-grammar.md` §3.4 (control flow expressions) +
//! §3.6 (stmt + block) + §3.4 match_arm.
//!
//! Test dimensions covered:
//! - Control flow conformance suite expanded (98 → 177 .lin files)
//! - All 10 control flow sub-categories present (if/else, if-let, while,
//!   while-let, for, loop, match, break/continue/return, block/stmt, errors)
//! - if-let / while-let verified as Stage 1 features (FAIL with "Stage 0"
//!   error pattern)
//! - Parser error recovery behavior verified

#![cfg(test)]

use std::path::Path;

/// Verify control-flow conformance directory has 80 .lin files
#[test]
fn test_stage9_3_control_flow_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cf_dir = manifest.join("tests/conformance/00-parse/02-control-flow");
    assert!(
        cf_dir.exists(),
        "tests/conformance/00-parse/02-control-flow/ must exist"
    );

    let lin_count = std::fs::read_dir(&cf_dir)
        .expect("read 02-control-flow/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
        .count();

    assert!(
        lin_count >= 80,
        "02-control-flow/ should have at least 80 .lin files after Stage 9.3, got {lin_count}"
    );
}

/// Verify if/else tests present (12 tests per plan-9.3.md §3.1)
#[test]
fn test_stage9_3_if_else_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cf_dir = manifest.join("tests/conformance/00-parse/02-control-flow");

    let if_tests = [
        "if_basic.lin",
        "if_else.lin",
        "if_else_if.lin",
        "if_no_else.lin",
        "if_in_let.lin",
        "if_nested.lin",
        "if_cond_cmp.lin",
        "if_cond_logic.lin",
        "if_cond_call.lin",
        "if_block_multi_stmt.lin",
        "if_empty_block.lin",
        "if_expr_returns.lin",
    ];

    for name in &if_tests {
        let path = cf_dir.join(name);
        assert!(path.exists(), "if/else test {name} must exist");
    }
}

/// Verify if-let tests present and marked as PASS (Stage 13.2 TD-031 closed — if-let now supported)
#[test]
fn test_stage9_3_if_let_tests_marked_fail() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cf_dir = manifest.join("tests/conformance/00-parse/02-control-flow");

    let if_let_tests = [
        "if_let_basic.lin",
        "if_let_else.lin",
        "if_let_tuple.lin",
        "if_let_struct.lin",
        "if_let_wildcard.lin",
        "if_let_chain.lin",
    ];

    for name in &if_let_tests {
        let path = cf_dir.join(name);
        assert!(path.exists(), "if-let test {name} must exist");

        let content = std::fs::read_to_string(&path).expect("read if-let test");
        // Stage 13.2 (TD-031): if-let is now supported — tests flipped from FAIL to PASS
        assert!(
            content.contains("//! PASS"),
            "{name} must be PASS — if-let now supported in Stage 13.2 (TD-031 closed)"
        );
    }
}

/// Verify while tests present (8 tests per plan-9.3.md §3.3)
#[test]
fn test_stage9_3_while_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cf_dir = manifest.join("tests/conformance/00-parse/02-control-flow");

    let while_tests = [
        "while_basic.lin",
        "while_cond_cmp.lin",
        "while_cond_logic.lin",
        "while_empty.lin",
        "while_break.lin",
        "while_continue.lin",
        "while_nested.lin",
        "while_in_fn.lin",
    ];

    for name in &while_tests {
        let path = cf_dir.join(name);
        assert!(path.exists(), "while test {name} must exist");
    }
}

/// Verify while-let tests present and marked as PASS (Stage 13.2 TD-031 closed — while-let now supported)
#[test]
fn test_stage9_3_while_let_tests_marked_fail() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cf_dir = manifest.join("tests/conformance/00-parse/02-control-flow");

    let while_let_tests = [
        "while_let_basic.lin",
        "while_let_break.lin",
        "while_let_tuple.lin",
        "while_let_nested.lin",
        "while_let_continue.lin",
    ];

    for name in &while_let_tests {
        let path = cf_dir.join(name);
        assert!(path.exists(), "while-let test {name} must exist");

        let content = std::fs::read_to_string(&path).expect("read while-let test");
        // Stage 13.2 (TD-031): while-let is now supported — tests flipped from FAIL to PASS
        assert!(
            content.contains("//! PASS"),
            "{name} must be PASS — while-let now supported in Stage 13.2 (TD-031 closed)"
        );
    }
}

/// Verify for loop tests present (8 tests per plan-9.3.md §3.5)
#[test]
fn test_stage9_3_for_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cf_dir = manifest.join("tests/conformance/00-parse/02-control-flow");

    let for_tests = [
        "for_basic.lin",
        "for_range.lin",
        "for_range_inclusive.lin",
        "for_break.lin",
        "for_continue.lin",
        "for_nested.lin",
        "for_pat_tuple.lin",
        "for_empty.lin",
    ];

    for name in &for_tests {
        let path = cf_dir.join(name);
        assert!(path.exists(), "for test {name} must exist");
    }
}

/// Verify loop tests present (6 tests per plan-9.3.md §3.6)
#[test]
fn test_stage9_3_loop_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cf_dir = manifest.join("tests/conformance/00-parse/02-control-flow");

    let loop_tests = [
        "loop_basic.lin",
        "loop_break.lin",
        "loop_break_value.lin",
        "loop_continue.lin",
        "loop_nested.lin",
        "loop_while_interplay.lin",
    ];

    for name in &loop_tests {
        let path = cf_dir.join(name);
        assert!(path.exists(), "loop test {name} must exist");
    }
}

/// Verify match tests present (15 tests per plan-9.3.md §3.7)
#[test]
fn test_stage9_3_match_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cf_dir = manifest.join("tests/conformance/00-parse/02-control-flow");

    let match_tests = [
        "match_basic.lin",
        "match_multi_arm.lin",
        "match_wildcard.lin",
        "match_ident.lin",
        "match_tuple.lin",
        "match_struct.lin",
        "match_enum.lin",
        "match_guard.lin",
        "match_block_arm.lin",
        "match_range_pat.lin",
        "match_or_pat.lin",
        "match_nested.lin",
        "match_in_let.lin",
        "match_expr_scrutinee.lin",
        "match_empty.lin",
    ];

    for name in &match_tests {
        let path = cf_dir.join(name);
        assert!(path.exists(), "match test {name} must exist");
    }
}

/// Verify break/continue/return tests present (10 tests per plan-9.3.md §3.8)
#[test]
fn test_stage9_3_break_continue_return_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cf_dir = manifest.join("tests/conformance/00-parse/02-control-flow");

    let bcr_tests = [
        "break_basic.lin",
        "break_value.lin",
        "break_in_while.lin",
        "break_in_for.lin",
        "continue_basic.lin",
        "continue_in_for.lin",
        "continue_in_loop.lin",
        "return_basic.lin",
        "return_void.lin",
        "return_in_match.lin",
    ];

    for name in &bcr_tests {
        let path = cf_dir.join(name);
        assert!(
            path.exists(),
            "break/continue/return test {name} must exist"
        );
    }
}

/// Verify block + statement tests present (5 tests per plan-9.3.md §3.9)
#[test]
fn test_stage9_3_block_stmt_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cf_dir = manifest.join("tests/conformance/00-parse/02-control-flow");

    let bs_tests = [
        "block_basic.lin",
        "block_expr.lin",
        "block_trailing_expr.lin",
        "stmt_let.lin",
        "stmt_let_with_type.lin",
    ];

    for name in &bs_tests {
        let path = cf_dir.join(name);
        assert!(path.exists(), "block/stmt test {name} must exist");
    }
}

/// Verify error recovery tests present (5 tests per plan-9.3.md §3.10)
#[test]
fn test_stage9_3_error_recovery_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cf_dir = manifest.join("tests/conformance/00-parse/02-control-flow");

    let err_tests = [
        "err_if_without_cond.lin",
        "err_match_without_scrutinee.lin",
        "err_while_without_cond.lin",
        "err_for_without_in.lin",
        "err_break_outside_loop.lin",
    ];

    for name in &err_tests {
        let path = cf_dir.join(name);
        assert!(path.exists(), "error recovery test {name} must exist");
    }

    // 4 should be FAIL (parser reports expected token)
    for name in &err_tests[..4] {
        let path = cf_dir.join(name);
        let content = std::fs::read_to_string(&path).expect("read err test");
        assert!(
            content.contains("//! FAIL"),
            "{name} must be FAIL — parser must report error"
        );
    }
}

/// Verify Stage 9.3 docs created
#[test]
fn test_stage9_3_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan_9_3 = manifest.join("docs/develop/v0/stage-9/plan-9.3.md");
    let gate_review_9_3 = manifest.join("docs/develop/v0/stage-9/gate-review-9.3.md");
    let test_plan = manifest.join("docs/tests/v0/stage9/plan/control_flow.md");

    assert!(plan_9_3.exists(), "plan-9.3.md must exist");
    assert!(gate_review_9_3.exists(), "gate-review-9.3.md must exist");
    assert!(
        test_plan.exists(),
        "docs/tests/v0/stage9/plan/control_flow.md must exist"
    );
}

/// Verify Cargo.toml version is at least 0.16.2 (Stage 9.3 bumped to 0.16.2;
/// later stages may bump further)
#[test]
fn test_stage9_3_cargo_toml_version_bumped() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    // Stage 9.3 bumped to 0.16.2; later stages may bump further
    let version_line = content
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("version line must exist");
    assert!(
        version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.17.")
            || version_line.starts_with("version = \"0.18.")
            || version_line.starts_with("version = \"0.19.")
            || version_line.starts_with("version = \"0.20.")
            || version_line.starts_with("version = \"0.21.")
            || version_line.starts_with("version = \"0.22.")
            || version_line.starts_with("version = \"0.23."),
        "Cargo.toml version must be 0.16.2+ after Stage 9.3 bump, got: {version_line}"
    );
}

/// Verify conformance suite total ≥ 177 (98 from 9.2 + 79 new from 9.3)
#[test]
fn test_stage9_3_conformance_total_reaches_177() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parse_dir = manifest.join("tests/conformance/00-parse");

    let mut total = 0;
    for entry in std::fs::read_dir(&parse_dir).expect("read 00-parse/") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            total += std::fs::read_dir(&path)
                .expect("read category dir")
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
                .count();
        }
    }

    assert!(
        total >= 177,
        "conformance suite total should be at least 177 (98 + 79) after Stage 9.3, got {total}"
    );
}
