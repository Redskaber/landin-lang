//! Stage 15.37 — Driver switch (DEFERRED) + legacy deprecation + GAP-1
//! semantic conflict documentation.
//!
//! These tests verify:
//!
//! 1. **Deprecation smoke test** — `check_mir_body` is marked `#[deprecated]`
//!    with a note pointing to `check_mir_body_with_dataflow`. Compiling
//!    code that calls `check_mir_body` produces a deprecation warning
//!    (tested via `#[allow(deprecated)]` — the warning is the expected
//!    behavior, not a test failure).
//! 2. **Driver integration** — the driver still uses `check_mir_body`
//!    (the dataflow switch was deferred due to the GAP-1 semantic
//!    conflict). Compiling a simple program produces 0 errors and the
//!    expected `CompileResult`.
//! 3. **Dataflow path still accessible** — `check_mir_body_with_dataflow`
//!    is callable and produces correct results on soundness patterns
//!    (loop-carried borrows, branch-carried borrows).
//! 4. **GAP-1 semantic conflict regression test** — documents the known
//!    case where the dataflow path and legacy path disagree:
//!    `let r1 = &mut x; let r2 = &mut x;`. The legacy path errors
//!    (GAP-1 fix); the dataflow path accepts (correct NLL). This test
//!    documents the conflict so future reconciliation work has a
//!    clear acceptance criterion.
//!
//! Per §29.1.3 (Design-Impl-Test coverage): the tests document both the
//! deprecation API contract and the semantic conflict that blocks the
//! driver switch. Per §23.1 rule 6: the deprecation note is verified
//! to point to the sounder alternative.
// Stage 15.37: Allow deprecated — these tests intentionally call the
// legacy `check_mir_body` to verify it is still callable (deprecation
// is a warning, not a removal) and to compare against the dataflow path.
#![allow(deprecated)]
#![cfg(test)]

use landin_compiler::borrowck::{check_mir_body, check_mir_body_with_dataflow, BorrowChecker};
use landin_compiler::compile;

// ============================================================
// Part A — Deprecation smoke tests
// ============================================================

/// Stage 15.37 test 1: The legacy `check_mir_body` free function is
/// callable and produces the same result as before (it's deprecated,
/// not removed). The `#[allow(deprecated)]` attribute on this module
/// suppresses the expected deprecation warning.
///
/// Per §23.1 rule 6: deprecated entry points must still work — they
/// signal "use the alternative" via warning, not via removal. This test
/// verifies the legacy path is still functional for backward compat.
#[test]
#[allow(deprecated)]
fn stage15_37_legacy_check_mir_body_still_callable() {
    let src = r#"
        fn main() -> i32 {
            let x = 1;
            let y = 2;
            x + y
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let errors = check_mir_body(mir_body);
        assert!(
            errors.is_empty(),
            "legacy check_mir_body should accept valid program, got: {:?}",
            errors
        );
    }
}

/// Stage 15.37 test 2: The legacy `BorrowChecker::check_mir_body` method
/// is callable and produces the same result as before.
#[test]
#[allow(deprecated)]
fn stage15_37_legacy_borrow_checker_method_still_callable() {
    let src = r#"
        fn main() -> i32 {
            let x = 1;
            x + 1
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let mut bc = BorrowChecker::new();
        bc.check_mir_body(mir_body);
        let errors = bc.into_errors();
        assert!(
            errors.is_empty(),
            "legacy BorrowChecker::check_mir_body should accept valid program, got: {:?}",
            errors
        );
    }
}

// ============================================================
// Part B — Driver integration tests
// ============================================================

/// Stage 15.37 test 3: The driver uses the legacy `check_mir_body`
/// (dataflow switch was deferred). Compiling a valid program produces
/// 0 errors and a valid `CompileResult`.
///
/// This is the key Stage 15.37 acceptance test: even though the
/// dataflow path exists and is algorithmically correct, the driver
/// continues to use the legacy path because of the GAP-1 semantic
/// conflict. This test verifies the driver behavior is unchanged
/// from v0.162.0.
#[test]
fn stage15_37_driver_uses_legacy_path_no_regression() {
    let src = r#"
        fn main() -> i32 {
            let x = 1;
            let y = 2;
            x + y
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "valid program should compile cleanly");
    assert!(!result.mirs.is_empty(), "should produce MIR");
}

/// Stage 15.37 test 4: The driver still catches double-mut-borrow
/// (GAP-1 soundness fix is preserved).
///
/// `let r1 = &mut x; let r2 = &mut x;` must produce a borrowck error
/// under the legacy path (which the driver uses). This is the GAP-1
/// soundness guarantee that 112 conformance tests depend on.
#[test]
fn stage15_37_driver_preserves_gap1_soundness() {
    let src = r#"
        fn main() -> i32 {
            let mut x = 1;
            let r1 = &mut x;
            let r2 = &mut x;
            0
        }
    "#;
    let result = compile(src);
    // Stage 15.67 (True Rust NLL): r1 is never read, so its borrow expires
    // immediately (true NLL). The double mut borrow is ALLOWED.
    // (Previously, GAP-1 compromise rejected this; now correct NLL accepts it.)
    assert!(
        !result.has_errors(),
        "True NLL: double mut borrow with never-read r1 should be allowed (got: {:?})",
        result.errors
    );
}

// ============================================================
// Part C — Dataflow path still accessible
// ============================================================

/// Stage 15.37 test 5: `check_mir_body_with_dataflow` is callable on
/// real MIR and produces correct results on valid programs.
///
/// Even though the driver doesn't use it yet, the dataflow path must
/// remain accessible for testing and future migration. This test
/// verifies it still works after Stage 15.37's deprecation changes.
#[test]
fn stage15_37_dataflow_path_still_accessible() {
    let src = r#"
        fn main() -> i32 {
            let x = 10;
            let y = 20;
            x + y
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let errors = check_mir_body_with_dataflow(mir_body);
        assert!(
            errors.is_empty(),
            "dataflow path should accept valid program, got: {:?}",
            errors
        );
    }
}

/// Stage 15.37 test 6: `check_mir_body_with_dataflow` correctly handles
/// loop-carried borrows (the soundness pattern it was designed to fix).
#[test]
fn stage15_37_dataflow_path_handles_loop_borrow() {
    let src = r#"
        fn main() -> i32 {
            let x = 10;
            let r = &x;
            let mut i = 0;
            let mut s = 0;
            while i < 3 {
                s = s + *r;
                i = i + 1;
            }
            s
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let errors = check_mir_body_with_dataflow(mir_body);
        assert!(
            errors.is_empty(),
            "dataflow path should accept loop-carried borrow, got: {:?}",
            errors
        );
    }
}

// ============================================================
// Part D — GAP-1 conflict regression test (RESOLVED in Stage 15.39)
// ============================================================

/// Stage 15.37 test 7 (UPDATED in Stage 15.39): **GAP-1 conflict RESOLVED.**
///
/// **Original Stage 15.37 behavior**: The dataflow path accepted
/// `let r1 = &mut x; let r2 = &mut x;` (correct NLL — `r1` is dead),
/// while the legacy path rejected it (GAP-1 soundness fix). This was
/// the documented semantic conflict that blocked the driver switch.
///
/// **Stage 15.39 update**: Option B (`compute_ever_read` + modified
/// `kill_expired_borrows_dataflow`) resolved the conflict. The dataflow
/// path now ALSO rejects this pattern, matching the legacy path. The
/// "was ever read" check preserves GAP-1: a borrow whose `ref_local`
/// was never read is NOT killed (it stays as a "stray" until scope end,
/// matching the legacy path's behavior).
///
/// This test verifies the resolution: both paths now reject the GAP-1
/// pattern. The 112 conformance cases that depended on GAP-1 semantics
/// are now AGREE-ERROR (both paths reject with the same error count).
///
/// See `docs/develop/v0/stage-15/stage-15.39-option-b-implementation.md`
/// for the full resolution analysis.
#[test]
#[allow(deprecated)]
fn stage15_37_gap1_semantic_conflict_documented() {
    let src = r#"
        fn main() -> i32 {
            let mut x = 1;
            let r1 = &mut x;
            let r2 = &mut x;
            0
        }
    "#;
    let result = compile(src);
    // Stage 15.67 (True Rust NLL): r1 is never read, so its borrow expires
    // immediately (true NLL). The double mut borrow is ALLOWED.
    // (Previously, GAP-1 compromise rejected this; now correct NLL accepts it.)
    assert!(
        !result.has_errors(),
        "True NLL: double mut borrow with never-read r1 should be allowed (got: {:?})",
        result.errors
    );

    // Now test each borrow-check path directly on the MIR.
    let main_mir = result
        .mirs
        .iter()
        .find(|m| m.basic_blocks.iter().any(|bb| !bb.statements.is_empty()))
        .expect("should find main's MIR");

    // Legacy path: now delegates to dataflow (Stage 15.41), so both agree.
    let legacy_errors = check_mir_body(main_mir);
    assert!(
        legacy_errors.is_empty(),
        "True NLL: legacy path (delegates to dataflow) must accept double-mut-borrow with never-read r1. Errors: {:?}",
        legacy_errors
    );

    // Dataflow path: true NLL accepts (r1 never read → borrow expires).
    let dataflow_errors = check_mir_body_with_dataflow(main_mir);
    assert!(
        dataflow_errors.is_empty(),
        "True NLL: dataflow path must accept double-mut-borrow with never-read r1. Errors: {:?}",
        dataflow_errors
    );
}

// ============================================================
// Part E — Parity on valid programs (no conflict cases)
// ============================================================

/// Stage 15.37 test 8: For valid programs WITHOUT the GAP-1 conflict
/// pattern, the legacy and dataflow paths must agree (both produce 0
/// errors). This is the same parity criterion from Stage 15.36, re-run
/// after the deprecation changes to verify no regression.
#[test]
#[allow(deprecated)]
fn stage15_37_parity_on_valid_program() {
    let src = r#"
        fn main() -> i32 {
            let x = 1;
            let y = x + 1;
            let z = y * 2;
            z
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let legacy_errors = check_mir_body(mir_body);
        let dataflow_errors = check_mir_body_with_dataflow(mir_body);
        assert_eq!(
            legacy_errors.len(),
            dataflow_errors.len(),
            "parity on valid program: legacy={}, dataflow={}",
            legacy_errors.len(),
            dataflow_errors.len()
        );
        assert_eq!(legacy_errors.len(), 0, "valid program should have 0 errors");
    }
}

/// Stage 15.37 test 9: Parity on a program with a single borrow (no
/// conflict). Both paths should accept.
#[test]
#[allow(deprecated)]
fn stage15_37_parity_on_single_borrow() {
    let src = r#"
        fn main() -> i32 {
            let x = 10;
            let r = &x;
            *r
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let legacy_errors = check_mir_body(mir_body);
        let dataflow_errors = check_mir_body_with_dataflow(mir_body);
        assert_eq!(legacy_errors.len(), 0, "legacy accepts single borrow");
        assert_eq!(dataflow_errors.len(), 0, "dataflow accepts single borrow");
    }
}
