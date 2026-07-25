//! Stage 9.8 — Closures conformance expansion verification tests
//!
//! Per stage-committee-process.md v3.21 §17.1 + §13.4 design alignment
//! with `docs/lang-design/02-grammar.md` §3.4 (closure forms) + §4.2
//! (closure vs binary OR disambiguation).
//!
//! Test dimensions covered:
//! - Closures conformance suite expanded (397 → 437 .lin files)
//! - All 7 closure sub-categories present (basic/move/captures/args/return/
//!   disambiguation/error-recovery)
//! - Parser limitations documented (closure type syntax || -> i32)

#![cfg(test)]

use std::path::Path;

/// Verify closures conformance directory has 40+ .lin files
#[test]
fn test_stage9_8_closures_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let clos_dir = manifest.join("tests/conformance/00-parse/07-closures");
    assert!(
        clos_dir.exists(),
        "tests/conformance/00-parse/07-closures/ must exist"
    );

    let lin_count = std::fs::read_dir(&clos_dir)
        .expect("read 07-closures/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
        .count();

    assert!(
        lin_count >= 40,
        "07-closures/ should have at least 40 .lin files after Stage 9.8, got {lin_count}"
    );
}

/// Verify basic closure tests present (10 tests per plan-9.8.md §3.1)
#[test]
fn test_stage9_8_basic_closure_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let clos_dir = manifest.join("tests/conformance/00-parse/07-closures");

    let basic_tests = [
        "closure_empty.lin",
        "closure_empty_block.lin",
        "closure_single_param.lin",
        "closure_single_param_block.lin",
        "closure_multi_params.lin",
        "closure_typed_param.lin",
        "closure_typed_multi.lin",
        "closure_in_let.lin",
        "closure_call.lin",
        "closure_nested.lin",
    ];

    for name in &basic_tests {
        let path = clos_dir.join(name);
        assert!(path.exists(), "basic closure test {name} must exist");
    }
}

/// Verify move closure tests present (8 tests per plan-9.8.md §3.2)
#[test]
fn test_stage9_8_move_closure_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let clos_dir = manifest.join("tests/conformance/00-parse/07-closures");

    let move_tests = [
        "closure_move_empty.lin",
        "closure_move_param.lin",
        "closure_move_block.lin",
        "closure_move_multi.lin",
        "closure_move_typed.lin",
        "closure_move_in_let.lin",
        "closure_move_capture.lin",
        "closure_move_nested.lin",
    ];

    for name in &move_tests {
        let path = clos_dir.join(name);
        assert!(path.exists(), "move closure test {name} must exist");
    }
}

/// Verify closure capture tests present (7 tests per plan-9.8.md §3.3)
#[test]
fn test_stage9_8_capture_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let clos_dir = manifest.join("tests/conformance/00-parse/07-closures");

    let capture_tests = [
        "closure_capture_ref.lin",
        "closure_capture_mut.lin",
        "closure_capture_multi.lin",
        "closure_capture_move.lin",
        "closure_capture_in_fn.lin",
        "closure_capture_nested.lin",
        "closure_capture_string.lin",
    ];

    for name in &capture_tests {
        let path = clos_dir.join(name);
        assert!(path.exists(), "capture test {name} must exist");
    }
}

/// Verify closure as argument tests present (5 tests per plan-9.8.md §3.4)
/// 1 should be FAIL (closure type syntax not supported)
#[test]
fn test_stage9_8_arg_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let clos_dir = manifest.join("tests/conformance/00-parse/07-closures");

    let arg_tests = [
        "closure_arg_basic.lin",
        "closure_arg_call.lin",
        "closure_arg_pass.lin",
        "closure_arg_inline.lin",
        "closure_arg_move.lin",
    ];

    for name in &arg_tests {
        let path = clos_dir.join(name);
        assert!(path.exists(), "closure arg test {name} must exist");
    }

    // closure_arg_basic should be FAIL (closure type syntax || -> i32 not supported)
    let content = std::fs::read_to_string(clos_dir.join("closure_arg_basic.lin"))
        .expect("read closure_arg_basic.lin");
    assert!(
        content.contains("//! FAIL"),
        "closure_arg_basic.lin must be FAIL — closure type syntax || -> i32 not supported in Stage 0"
    );
}

/// Verify closure return type tests present (5 tests per plan-9.8.md §3.5)
#[test]
fn test_stage9_8_return_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let clos_dir = manifest.join("tests/conformance/00-parse/07-closures");

    let ret_tests = [
        "closure_ret_unit.lin",
        "closure_ret_int.lin",
        "closure_ret_ref.lin",
        "closure_ret_closure.lin",
        "closure_ret_block.lin",
    ];

    for name in &ret_tests {
        let path = clos_dir.join(name);
        assert!(path.exists(), "closure return test {name} must exist");
    }
}

/// Verify closure disambiguation tests present (3 tests per plan-9.8.md §3.6)
#[test]
fn test_stage9_8_disambiguation_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let clos_dir = manifest.join("tests/conformance/00-parse/07-closures");

    let disamb_tests = [
        "closure_vs_bitor.lin",
        "closure_in_match.lin",
        "closure_chain.lin",
    ];

    for name in &disamb_tests {
        let path = clos_dir.join(name);
        assert!(path.exists(), "disambiguation test {name} must exist");
    }
}

/// Verify error recovery tests present (2 tests per plan-9.8.md §3.7)
#[test]
fn test_stage9_8_error_recovery_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let clos_dir = manifest.join("tests/conformance/00-parse/07-closures");

    let err_tests = ["err_closure_unclosed.lin", "err_closure_no_body.lin"];

    for name in &err_tests {
        let path = clos_dir.join(name);
        assert!(path.exists(), "error recovery test {name} must exist");
    }

    // Both should be PASS (parser accepts via synthetic node recovery)
    for name in &err_tests {
        let path = clos_dir.join(name);
        let content = std::fs::read_to_string(&path).expect("read err test");
        assert!(
            content.contains("//! PASS"),
            "{name} must be PASS — parser accepts via synthetic node recovery"
        );
    }
}

/// Verify Stage 9.8 docs created
#[test]
fn test_stage9_8_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan_9_8 = manifest.join("docs/develop/v0/stage-9/plan-9.8.md");
    let gate_review_9_8 = manifest.join("docs/develop/v0/stage-9/gate-review-9.8.md");
    let test_plan = manifest.join("docs/tests/v0/stage9/plan/closures.md");

    assert!(plan_9_8.exists(), "plan-9.8.md must exist");
    assert!(gate_review_9_8.exists(), "gate-review-9.8.md must exist");
    assert!(
        test_plan.exists(),
        "docs/tests/v0/stage9/plan/closures.md must exist"
    );
}

/// Verify Cargo.toml version bumped to 0.16.7+
#[test]
fn test_stage9_8_cargo_toml_version_bumped() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    let version_line = content
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("version line must exist");
    assert!(
        version_line.starts_with("version = \"0.16.7")
            || version_line.starts_with("version = \"0.16.8")
            || version_line.starts_with("version = \"0.16.9")
            || version_line.starts_with("version = \"0.16.10")
            || version_line.starts_with("version = \"0.16.11")
            || version_line.starts_with("version = \"0.16.12")
            || version_line.starts_with("version = \"0.17.")
            || version_line.starts_with("version = \"0.18.")
            || version_line.starts_with("version = \"0.19.")
            || version_line.starts_with("version = \"0.20."),
        "Cargo.toml version must be 0.16.7+ after Stage 9.8 bump, got: {version_line}"
    );
}

/// Verify conformance suite total ≥ 437 (397 from 9.7 + 40 new from 9.8)
#[test]
fn test_stage9_8_conformance_total_reaches_437() {
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
        total >= 437,
        "conformance suite total should be at least 437 (397 + 40) after Stage 9.8, got {total}"
    );
}
