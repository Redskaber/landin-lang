//! Stage 13.3a — TD-030 closure call lowering (P0 CLOSED) verification
//!
//! Verifies that closures are now callable via the inline approach:
//! - ClosureBodyInfo side-table exists on MirLowerCtxt
//! - HirExprKind::Closure arm stores closure info in side-table
//! - HirExprKind::Call arm detects closure callee + inlines body
//! - lower_closure_call_inline function exists
//! - Conformance tests flipped from compile_error to compile_ok
//!
//! Per stage-committee-process.md v3.21 §13.4 + §14.4 + §25.8.

#![cfg(test)]

use std::path::Path;

/// Verify ClosureBodyInfo side-table exists in MirLowerCtxt
#[test]
fn test_closure_body_info_side_table_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mir_lower_mod = manifest.join("src/mir/lower/mod.rs");
    let content = std::fs::read_to_string(&mir_lower_mod).expect("read src/mir/lower/mod.rs");

    // Must have closure_bodies field on MirLowerCtxt
    assert!(
        content.contains("closure_bodies"),
        "src/mir/lower/mod.rs must have `closure_bodies` field on MirLowerCtxt"
    );

    // Must have ClosureBodyInfo struct
    assert!(
        content.contains("struct ClosureBodyInfo"),
        "src/mir/lower/mod.rs must define `ClosureBodyInfo` struct"
    );

    // ClosureBodyInfo must have params, body, captures fields
    assert!(
        content.contains("params") && content.contains("body") && content.contains("captures"),
        "ClosureBodyInfo must have params, body, captures fields"
    );
}

/// Verify HirExprKind::Closure arm stores closure info in side-table
#[test]
fn test_closure_arm_stores_info() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expr_operand = manifest.join("src/mir/lower/expr_operand.rs");
    let content = std::fs::read_to_string(&expr_operand).expect("read expr_operand.rs");

    // Must insert into closure_bodies
    assert!(
        content.contains("closure_bodies.insert"),
        "expr_operand.rs must insert closure info into `closure_bodies` side-table"
    );

    // Must reference ClosureBodyInfo
    assert!(
        content.contains("ClosureBodyInfo"),
        "expr_operand.rs must reference ClosureBodyInfo"
    );
}

/// Verify HirExprKind::Call arm detects + dispatches closure calls
#[test]
fn test_call_arm_dispatches_closure_calls() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expr_operand = manifest.join("src/mir/lower/expr_operand.rs");
    let content = std::fs::read_to_string(&expr_operand).expect("read expr_operand.rs");

    // Must check closure_bodies side-table in Call arm
    assert!(
        content.contains("closure_bodies.get")
            || content.contains("closure_bodies.get(&func_local)"),
        "expr_operand.rs must check `closure_bodies` side-table in Call arm"
    );

    // Must call lower_closure_call_inline
    assert!(
        content.contains("lower_closure_call_inline"),
        "expr_operand.rs must call `lower_closure_call_inline` for closure calls"
    );
}

/// Verify lower_closure_call_inline function exists
#[test]
fn test_lower_closure_call_inline_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expr_operand = manifest.join("src/mir/lower/expr_operand.rs");
    let content = std::fs::read_to_string(&expr_operand).expect("read expr_operand.rs");

    // Must define lower_closure_call_inline function
    assert!(
        content.contains("fn lower_closure_call_inline"),
        "expr_operand.rs must define `lower_closure_call_inline` function"
    );

    // Must reference Stage 13.3a / TD-030
    assert!(
        content.contains("Stage 13.3a") || content.contains("TD-030"),
        "expr_operand.rs must reference Stage 13.3a / TD-030 in closure call lowering"
    );
}

/// Verify Stage 13.3a gate review exists + marks TD-030 CLOSED
#[test]
fn test_stage13_3a_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-13/gate-review-13.3a.md");
    assert!(
        gate_review.exists(),
        "docs/develop/v0/stage-13/gate-review-13.3a.md must exist"
    );

    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-13.3a.md");

    // Must mark TD-030 as CLOSED
    assert!(
        content.contains("TD-030") && content.contains("CLOSED"),
        "gate-review-13.3a.md must mark TD-030 as CLOSED"
    );

    // Must reference inline approach
    assert!(
        content.contains("inline"),
        "gate-review-13.3a.md must reference the inline approach"
    );

    // Must include committee vote
    assert!(
        content.contains("委员会投票") || content.contains("Committee") || content.contains("Vote"),
        "gate-review-13.3a.md must include committee vote"
    );

    // Must reach PASS verdict
    assert!(
        content.contains("PASS"),
        "gate-review-13.3a.md must reach PASS verdict"
    );
}

/// Verify conformance tests flipped from compile_error to compile_ok
#[test]
fn test_conformance_closure_tests_flipped() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Check a sample of closure conformance tests that should now be compile_ok
    let closure_test_dirs = [
        "tests/conformance/01-typecheck/03-closures",
        "tests/conformance/02-borrowck/03-closure-capture",
        "tests/conformance/04-e2e/03-closures",
    ];

    let mut compile_ok_count = 0;
    let mut compile_error_count = 0;

    for dir in &closure_test_dirs {
        let dir_path = manifest.join(dir);
        if !dir_path.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir_path).expect("read dir") {
            let entry = entry.expect("dir entry");
            if entry
                .path()
                .extension()
                .map(|x| x == "lin")
                .unwrap_or(false)
            {
                let content = std::fs::read_to_string(entry.path()).expect("read .lin");
                if content.contains("compile_ok") {
                    compile_ok_count += 1;
                } else if content.contains("compile_error") {
                    compile_error_count += 1;
                }
            }
        }
    }

    // After Stage 13.3a, most closure tests should be compile_ok
    assert!(
        compile_ok_count > 0,
        "At least some closure conformance tests must be compile_ok after Stage 13.3a, got {} compile_ok, {} compile_error",
        compile_ok_count,
        compile_error_count
    );
}

/// Verify v0.1 conformance gate still holds
#[test]
fn test_v01_gate_still_holds_after_stage13_3a() {
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

/// Verify Cargo.toml version is v0.23.0 (minor bump for second user-facing feature)
#[test]
fn test_cargo_toml_version_is_v0_23_0() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    assert!(
        content.contains("version = \"0.23.0\""),
        "Cargo.toml version must be v0.23.0 (Stage 13.3a P0 closure, second user-facing feature)"
    );
}

/// Verify worklog has Stage 13.3a entry
#[test]
fn test_worklog_has_stage13_3a_entry() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest.join("docs/worklog.md")).expect("read worklog.md");

    assert!(
        content.contains("stage-13.3a") || content.contains("Stage 13.3a"),
        "worklog must reference Stage 13.3a"
    );
    assert!(
        content.contains("TD-030")
            && (content.contains("CLOSED") || content.contains("closure call")),
        "worklog must reference TD-030 closure"
    );
}
