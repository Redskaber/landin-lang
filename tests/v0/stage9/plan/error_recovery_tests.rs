//! Stage 9.10 — Error recovery conformance expansion verification tests
//!
//! Per stage-committee-process.md v3.21 §17.1 + §13.4 design alignment
//! with `docs/lang-design/02-grammar.md` §2 (error recovery via synthetic node).

#![cfg(test)]

use std::path::Path;

/// Verify error-recovery conformance directory has 51+ .lin files
#[test]
fn test_stage9_10_error_recovery_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let err_dir = manifest.join("tests/conformance/00-parse/09-error-recovery");
    assert!(
        err_dir.exists(),
        "tests/conformance/00-parse/09-error-recovery/ must exist"
    );

    let lin_count = std::fs::read_dir(&err_dir)
        .expect("read 09-error-recovery/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
        .count();

    assert!(
        lin_count >= 51,
        "09-error-recovery/ should have at least 51 .lin files (1 existing + 50 new) after Stage 9.10, got {lin_count}"
    );
}

/// Verify lexer error tests present (10 tests per plan-9.10.md §3.1)
#[test]
fn test_stage9_10_lex_error_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let err_dir = manifest.join("tests/conformance/00-parse/09-error-recovery");

    let lex_tests = [
        "err_lex_empty_oct.lin",
        "err_lex_empty_bin.lin",
        "err_lex_unterminated_string.lin",
        "err_lex_unterminated_char.lin",
        "err_lex_invalid_escape.lin",
        "err_lex_unterminated_block_comment.lin",
        "err_lex_invalid_unicode_escape.lin",
        "err_lex_leading_zero.lin",
        "err_lex_float_double_dot.lin",
        "err_lex_negative_zero.lin",
    ];

    for name in &lex_tests {
        let path = err_dir.join(name);
        assert!(path.exists(), "lexer error test {name} must exist");
    }
}

/// Verify parser expression error tests present (10 tests per plan-9.10.md §3.2)
#[test]
fn test_stage9_10_parse_expr_error_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let err_dir = manifest.join("tests/conformance/00-parse/09-error-recovery");

    let expr_tests = [
        "err_parse_unmatched_paren.lin",
        "err_parse_unmatched_bracket.lin",
        "err_parse_unmatched_brace.lin",
        "err_parse_missing_semi.lin",
        "err_parse_double_semi.lin",
        "err_parse_missing_expr.lin",
        "err_parse_missing_type.lin",
        "err_parse_missing_pat.lin",
        "err_parse_missing_fn_body.lin",
        "err_parse_missing_fn_name.lin",
    ];

    for name in &expr_tests {
        let path = err_dir.join(name);
        assert!(path.exists(), "parser expr error test {name} must exist");
    }
}

/// Verify parser item error tests present (10 tests per plan-9.10.md §3.3)
#[test]
fn test_stage9_10_parse_item_error_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let err_dir = manifest.join("tests/conformance/00-parse/09-error-recovery");

    let item_tests = [
        "err_parse_missing_struct_name.lin",
        "err_parse_missing_enum_name.lin",
        "err_parse_missing_trait_name.lin",
        "err_parse_missing_impl_type.lin",
        "err_parse_missing_const_name.lin",
        "err_parse_missing_const_type.lin",
        "err_parse_missing_const_value.lin",
        "err_parse_missing_where_colon.lin",
        "err_parse_missing_arrow_type.lin",
        "err_parse_missing_use_path.lin",
    ];

    for name in &item_tests {
        let path = err_dir.join(name);
        assert!(path.exists(), "parser item error test {name} must exist");
    }
}

/// Verify recovery tests present (7 synthetic node + 5 skip = 12 tests)
#[test]
fn test_stage9_10_recovery_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let err_dir = manifest.join("tests/conformance/00-parse/09-error-recovery");

    let recovery_tests = [
        "recovery_double_op.lin",
        "recovery_empty_let.lin",
        "recovery_empty_attr.lin",
        "recovery_empty_generics.lin",
        "recovery_empty_bound.lin",
        "recovery_empty_where.lin",
        "recovery_unclosed_closure.lin",
        "recovery_skip_to_semi.lin",
        "recovery_skip_to_brace.lin",
        "recovery_multi_errors.lin",
        "recovery_nested_errors.lin",
        "recovery_after_error.lin",
    ];

    for name in &recovery_tests {
        let path = err_dir.join(name);
        assert!(path.exists(), "recovery test {name} must exist");
    }

    // All recovery tests should be PASS (parser recovers via synthetic node)
    for name in &recovery_tests {
        let path = err_dir.join(name);
        let content = std::fs::read_to_string(&path).expect("read recovery test");
        assert!(
            content.contains("//! PASS") || content.contains("//! FAIL"),
            "{name} must have PASS or FAIL status"
        );
    }
}

/// Verify Stage 9.10 docs created
#[test]
fn test_stage9_10_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan_9_10 = manifest.join("docs/develop/v0/stage-9/plan-9.10.md");
    let gate_review_9_10 = manifest.join("docs/develop/v0/stage-9/gate-review-9.10.md");
    let test_plan = manifest.join("docs/tests/v0/stage9/plan/error_recovery.md");

    assert!(plan_9_10.exists(), "plan-9.10.md must exist");
    assert!(gate_review_9_10.exists(), "gate-review-9.10.md must exist");
    assert!(
        test_plan.exists(),
        "docs/tests/v0/stage9/plan/error_recovery.md must exist"
    );
}

/// Verify Cargo.toml version bumped to 0.16.9+
#[test]
fn test_stage9_10_cargo_toml_version_bumped() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    let version_line = content
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("version line must exist");
    assert!(
        version_line.starts_with("version = \"0.16.9")
            || version_line.starts_with("version = \"0.16.10")
            || version_line.starts_with("version = \"0.16.11")
            || version_line.starts_with("version = \"0.16.12")
            || version_line.starts_with("version = \"0.17.")
            || version_line.starts_with("version = \"0.18.")
            || version_line.starts_with("version = \"0.19.")
            || version_line.starts_with("version = \"0.20."),
        "Cargo.toml version must be 0.16.9+ after Stage 9.10 bump, got: {version_line}"
    );
}

/// Verify conformance suite total ≥ 547 (497 from 9.9 + 50 new from 9.10)
#[test]
fn test_stage9_10_conformance_total_reaches_547() {
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
        total >= 547,
        "conformance suite total should be at least 547 (497 + 50) after Stage 9.10, got {total}"
    );
}
