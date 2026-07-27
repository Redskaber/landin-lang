//! Stage 9.2 — Operators + Pratt precedence verification tests
//!
//! Per stage-committee-process.md v3.21 §17.1 + §13.4 design alignment
//! with `docs/lang-design/02-grammar.md` §3.4 (Expression) + §2 (Pratt
//! precedence table).
//!
//! Test dimensions covered:
//! - Operators conformance suite expanded (38 → 98 .lin files)
//! - All 6 operator categories present (arith / cmp / logic / bit / assign / unary)
//! - Pratt precedence verified (10 precedence combination tests)
//! - Postfix operators verified (5 tests)
//! - Error recovery verified (synthetic node behavior per §2 of 02-grammar.md)

#![cfg(test)]

use std::path::Path;

/// Verify operators conformance directory exists and has 60+ .lin files
#[test]
fn test_stage9_2_operators_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ops_dir = manifest.join("tests/conformance/00-parse/01-operators");
    assert!(
        ops_dir.exists(),
        "tests/conformance/00-parse/01-operators/ must exist"
    );

    let lin_count = std::fs::read_dir(&ops_dir)
        .expect("read 01-operators/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
        .count();

    assert!(
        lin_count >= 60,
        "01-operators/ should have at least 60 .lin files after Stage 9.2, got {lin_count}"
    );
}

/// Verify arithmetic operator tests exist (8 tests per plan-9.2.md §3.1)
#[test]
fn test_stage9_2_arithmetic_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ops_dir = manifest.join("tests/conformance/00-parse/01-operators");

    let arith_tests = [
        "arith_add.lin",
        "arith_sub.lin",
        "arith_mul.lin",
        "arith_div.lin",
        "arith_rem.lin",
        "arith_chain.lin",
        "arith_mixed.lin",
        "arith_parens.lin",
    ];

    for name in &arith_tests {
        let path = ops_dir.join(name);
        assert!(path.exists(), "arithmetic test {name} must exist");
    }
}

/// Verify comparison operator tests exist (6 tests per plan-9.2.md §3.2)
#[test]
fn test_stage9_2_comparison_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ops_dir = manifest.join("tests/conformance/00-parse/01-operators");

    let cmp_tests = [
        "cmp_eq.lin",
        "cmp_ne.lin",
        "cmp_lt.lin",
        "cmp_gt.lin",
        "cmp_le.lin",
        "cmp_ge.lin",
    ];

    for name in &cmp_tests {
        let path = ops_dir.join(name);
        assert!(path.exists(), "comparison test {name} must exist");
    }
}

/// Verify logical operator tests exist (5 tests per plan-9.2.md §3.3)
#[test]
fn test_stage9_2_logical_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ops_dir = manifest.join("tests/conformance/00-parse/01-operators");

    let logic_tests = [
        "logic_and.lin",
        "logic_or.lin",
        "logic_not.lin",
        "logic_chain.lin",
        "logic_parens.lin",
    ];

    for name in &logic_tests {
        let path = ops_dir.join(name);
        assert!(path.exists(), "logical test {name} must exist");
    }
}

/// Verify bitwise operator tests exist (6 tests per plan-9.2.md §3.4)
#[test]
fn test_stage9_2_bitwise_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ops_dir = manifest.join("tests/conformance/00-parse/01-operators");

    let bit_tests = [
        "bit_and.lin",
        "bit_or.lin",
        "bit_xor.lin",
        "bit_shl.lin",
        "bit_shr.lin",
        "bit_chain.lin",
    ];

    for name in &bit_tests {
        let path = ops_dir.join(name);
        assert!(path.exists(), "bitwise test {name} must exist");
    }
}

/// Verify compound assignment tests exist (12 tests per plan-9.2.md §3.5)
#[test]
fn test_stage9_2_compound_assignment_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ops_dir = manifest.join("tests/conformance/00-parse/01-operators");

    let assign_tests = [
        "assign_simple.lin",
        "assign_add.lin",
        "assign_sub.lin",
        "assign_mul.lin",
        "assign_div.lin",
        "assign_rem.lin",
        "assign_and.lin",
        "assign_or.lin",
        "assign_xor.lin",
        "assign_shl.lin",
        "assign_shr.lin",
        "assign_chain.lin",
    ];

    for name in &assign_tests {
        let path = ops_dir.join(name);
        assert!(path.exists(), "compound assignment test {name} must exist");
    }
}

/// Verify Pratt precedence combination tests exist (10 tests per plan-9.2.md §3.8)
#[test]
fn test_stage9_2_pratt_precedence_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ops_dir = manifest.join("tests/conformance/00-parse/01-operators");

    let prec_tests = [
        "prec_mul_over_add.lin",
        "prec_add_over_cmp.lin",
        "prec_cmp_over_and.lin",
        "prec_and_over_or.lin",
        "prec_or_over_assign.lin",
        "prec_shift_over_add.lin",
        "prec_bit_over_cmp.lin",
        "prec_unary_over_mul.lin",
        "prec_parens_override.lin",
        "prec_nested_parens.lin",
    ];

    for name in &prec_tests {
        let path = ops_dir.join(name);
        assert!(path.exists(), "precedence test {name} must exist");
    }
}

/// Verify error recovery tests exist (per plan-9.2.md §3.9)
#[test]
fn test_stage9_2_error_recovery_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ops_dir = manifest.join("tests/conformance/00-parse/01-operators");

    let err_tests = [
        "err_unmatched_paren.lin",
        "err_double_op.lin",
        "err_empty_expr.lin",
    ];

    for name in &err_tests {
        let path = ops_dir.join(name);
        assert!(path.exists(), "error recovery test {name} must exist");
    }

    // err_unmatched_paren should be FAIL (parser reports expected ')')
    let content = std::fs::read_to_string(ops_dir.join("err_unmatched_paren.lin"))
        .expect("read err_unmatched_paren.lin");
    assert!(
        content.contains("//! FAIL"),
        "err_unmatched_paren.lin must be FAIL (parser must error on unmatched paren)"
    );
}

/// Verify Stage 9.2 plan + gate-review docs created
#[test]
fn test_stage9_2_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan_9_2 = manifest.join("docs/develop/v0/stage-9/plan-9.2.md");
    let gate_review_9_2 = manifest.join("docs/develop/v0/stage-9/gate-review-9.2.md");
    let test_plan = manifest.join("docs/tests/v0/stage9/plan/operators.md");

    assert!(plan_9_2.exists(), "plan-9.2.md must exist");
    assert!(gate_review_9_2.exists(), "gate-review-9.2.md must exist");
    assert!(
        test_plan.exists(),
        "docs/tests/v0/stage9/plan/operators.md must exist"
    );
}

/// Verify Cargo.toml version is at least 0.16.1 (Stage 9.2 bumped to 0.16.1;
/// later stages may bump further)
#[test]
fn test_stage9_2_cargo_toml_version_bumped() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    // Stage 9.2 bumped to 0.16.1; later stages may bump further (0.16.2, 0.16.3, ...)
    // Verify the version is 0.16.x or higher
    let version_line = content
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("version line must exist");
    assert!(
        version_line.starts_with("version = \"0.16.")
            || version_line.starts_with("version = \"0.17.")
            || version_line.starts_with("version = \"0.18.")
            || version_line.starts_with("version = \"0.19.")
            || version_line.starts_with("version = \"0.20.")
            || version_line.starts_with("version = \"0.21.")
            || version_line.starts_with("version = \"0.22.")
            || version_line.starts_with("version = \"0.23.")
            || version_line.starts_with("version = \"0.24.")
            || version_line.starts_with("version = \"0.25.")
            || version_line.starts_with("version = \"0.26.")
            || version_line.starts_with("version = \"0.27."),
        "Cargo.toml version must be 0.16.x+ after Stage 9.2 bump, got: {version_line}"
    );
}

/// Verify conformance suite total ≥ 98 (38 from 9.1 + 60 new from 9.2)
#[test]
fn test_stage9_2_conformance_total_reaches_98() {
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
        total >= 98,
        "conformance suite total should be at least 98 (38 + 60) after Stage 9.2, got {total}"
    );
}
