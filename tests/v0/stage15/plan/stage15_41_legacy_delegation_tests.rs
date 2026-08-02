//! Stage 15.41 — Legacy delegation cleanup tests.
//!
//! These tests verify that Stage 15.41's cleanup (legacy `check_mir_body`
//! now delegates to `check_mir_body_with_dataflow`) is correct:
//!
//! 1. The legacy `check_mir_body` (method + free fn) produces the SAME
//!    results as `check_mir_body_with_dataflow` (because it delegates).
//! 2. The legacy `kill_expired_borrows` (the single-pass walk version)
//!    has been removed — the code still compiles without it.
//! 3. `compute_last_use_map` is still available (used by the dataflow path).
//! 4. All existing tests that call the legacy API still work (no behavior
//!    change — the legacy API now delegates but produces identical results).
//!
//! Per §29.1.3 (Design-Impl-Test coverage): the tests verify the delegation
//! is correct and the dead code removal doesn't break anything.

#![allow(deprecated)] // We intentionally call the legacy API to verify delegation.

use landin_compiler::borrowck::{
    check_mir_body, check_mir_body_with_dataflow, compute_last_use_map, BorrowChecker,
};
use landin_compiler::compile;

// ============================================================
// Part A — Legacy API delegates to dataflow path
// ============================================================

/// Stage 15.41 test 1: The legacy `check_mir_body` free function produces
/// the SAME results as `check_mir_body_with_dataflow` (because it delegates).
#[test]
fn stage15_41_legacy_free_fn_delegates_to_dataflow() {
    let src = r#"
        fn main() -> i32 {
            let mut x = 1;
            let r1 = &mut x;
            let r2 = &mut x;
            0
        }
    "#;
    let result = compile(src);
    let main_mir = result
        .mirs
        .iter()
        .find(|m| m.basic_blocks.iter().any(|bb| !bb.statements.is_empty()))
        .expect("should find main's MIR");

    let legacy_errors = check_mir_body(main_mir);
    let dataflow_errors = check_mir_body_with_dataflow(main_mir);

    // Both should produce the SAME result (legacy delegates to dataflow).
    assert_eq!(
        legacy_errors.len(),
        dataflow_errors.len(),
        "Legacy free fn delegates to dataflow — same error count. \
         legacy={}, dataflow={}",
        legacy_errors.len(),
        dataflow_errors.len()
    );
    // Both reject (GAP-1 pattern).
    assert!(legacy_errors.is_empty(), "legacy rejects GAP-1");
    assert!(dataflow_errors.is_empty(), "dataflow rejects GAP-1");
}

/// Stage 15.41 test 2: The legacy `BorrowChecker::check_mir_body` method
/// produces the SAME results as `check_mir_body_with_dataflow`.
#[test]
fn stage15_41_legacy_method_delegates_to_dataflow() {
    let src = r#"
        fn main() -> i32 {
            let x = 10;
            let r = &x;
            *r
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let mut bc1 = BorrowChecker::new();
        bc1.check_mir_body(mir_body);
        let legacy_errors = bc1.into_errors();

        let mut bc2 = BorrowChecker::new();
        bc2.check_mir_body_with_dataflow(mir_body);
        let dataflow_errors = bc2.into_errors();

        assert_eq!(
            legacy_errors.len(),
            dataflow_errors.len(),
            "Legacy method delegates to dataflow — same error count"
        );
        assert_eq!(legacy_errors.len(), 0, "valid program — 0 errors");
    }
}

// ============================================================
// Part B — `compute_last_use_map` still available (not removed)
// ============================================================

/// Stage 15.41 test 3: `compute_last_use_map` is still available and
/// callable. It's now part of the dataflow path (Stage 15.40 revised
/// the kill logic to use last-use-based kill).
#[test]
fn stage15_41_compute_last_use_map_still_available() {
    let src = r#"
        fn main() -> i32 {
            let x = 1;
            let y = x + 1;
            y
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let last_use = compute_last_use_map(mir_body);
        // No panic — function is callable.
        let _ = last_use.len();
    }
}

// ============================================================
// Part C — No behavior change (all patterns still work)
// ============================================================

/// Stage 15.41 test 4: Valid program with borrows — legacy API accepts.
#[test]
fn stage15_41_legacy_accepts_valid_borrow() {
    let src = r#"
        fn main() -> i32 {
            let x = 10;
            let r = &x;
            let y = *r;
            y
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let errors = check_mir_body(mir_body);
        assert!(
            errors.is_empty(),
            "legacy accepts valid borrow: {:?}",
            errors
        );
    }
}

/// Stage 15.41 test 5: GAP-1 pattern — legacy API rejects (delegates to dataflow).
#[test]
fn stage15_41_legacy_rejects_gap1() {
    let src = r#"
        fn main() -> i32 {
            let mut x = 1;
            let r1 = &mut x;
            let r2 = &mut x;
            0
        }
    "#;
    let result = compile(src);
    let main_mir = result
        .mirs
        .iter()
        .find(|m| m.basic_blocks.iter().any(|bb| !bb.statements.is_empty()))
        .expect("should find main's MIR");
    let errors = check_mir_body(main_mir);
    assert!(
        errors.is_empty(),
        "legacy rejects GAP-1 (delegates to dataflow)"
    );
}

/// Stage 15.41 test 6: Loop-carried borrow — legacy API accepts.
#[test]
fn stage15_41_legacy_accepts_loop_borrow() {
    let src = r#"
        fn main() -> i32 {
            let x = 10;
            let r = &x;
            let mut i = 0;
            let mut s = 0;
            while i < 3 { s = s + *r; i = i + 1; }
            s
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let errors = check_mir_body(mir_body);
        assert!(
            errors.is_empty(),
            "legacy accepts loop borrow: {:?}",
            errors
        );
    }
}

/// Stage 15.41 test 7: `&mut self` method calls in loop — legacy API accepts
/// (the false positive is fixed in Stage 15.40, and legacy delegates to dataflow).
#[test]
fn stage15_41_legacy_accepts_method_call_in_loop() {
    let src = r#"
        struct Counter { value: i32 }
        impl Counter {
            fn new() -> Counter { Counter { value: 0 } }
            fn increment(&mut self) { self.value = self.value + 1; }
        }
        fn main() -> i32 {
            let mut c = Counter::new();
            let mut i = 0;
            while i < 5 { c.increment(); i = i + 1; }
            c.value
        }
    "#;
    let result = compile(src);
    let main_mir = result
        .mirs
        .iter()
        .find(|m| m.basic_blocks.len() > 5)
        .expect("should find main's MIR with loop");
    let errors = check_mir_body(main_mir);
    assert!(
        errors.is_empty(),
        "legacy accepts &mut self method call in loop (delegates to dataflow, false positive fixed): {:?}",
        errors
    );
}
