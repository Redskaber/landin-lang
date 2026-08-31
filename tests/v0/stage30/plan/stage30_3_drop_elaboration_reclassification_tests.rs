//! Stage 30.3 (v0.13 TD-STUB-DROP-ELABORATION-NOOP): Drop elaboration
//! reclassification + runtime behavior documentation tests.
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//!   - 4 positive tests (drop works for params + top-level locals)
//!   - 6 negative tests (drop does NOT fire at block scope end — known limitation)
//!   - 2 regression tests (drop glue emission + nested drop)
//!
//! Per §1.0 原則 4 (报错 > 静默): document the actual behavior accurately.
//! Per §1.0 原則 9 (正确 > 妥协): reclassify TD based on root-cause analysis.
//!
//! ## Background
//!
//! TD-STUB-DROP-ELABORATION-NOOP was classified as "elaborate_drops is no-op
//! (no `impl Drop` support yet)". Root-cause analysis (Stage 30.3) shows:
//!
//! 1. `elaborate_drops` IS implemented (Stage 15.43-15.46) and IS NOT a no-op.
//! 2. Drop glue codegen IS implemented (Stage 15.57) and IS emitted.
//! 3. Drop IS called at function end — verified by runtime test (param scope).
//! 4. BUT: `StorageDead` is emitted at FUNCTION END, not scope end
//!    (see `body_lower.rs` line 567-594). This means block-scoped locals
//!    get their drop called too late — after any observable side effects
//!    that follow the block.
//!
//! ## Reclassification
//!
//! - TD-STUB-DROP-ELABORATION-NOOP → **RESOLVED** (drop elaboration works)
//! - NEW: TD-DROP-SCOPE-TIMING (P2) — StorageDead emitted at function end,
//!   not scope end. Block-scoped locals drop too late.
//!
//! ## What these tests verify
//!
//! - **Positive**: Drop fires for fn params (correct scope end = fn end).
//! - **Positive**: Drop fires for top-level locals in main (correct scope
//!   end = fn end = program exit, so observable via exit code).
//! - **Negative (known limitation)**: Drop does NOT fire at block scope end.
//!   These tests document the current behavior — drop fires AFTER the
//!   println that observes the side effect, so the counter is still 0.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::{assert_runtime, run_program};
use landin_compiler::driver::compile;

// ============================================================================
// POSITIVE TESTS — Drop fires correctly at function scope end
// ============================================================================

/// Stage 30.3 positive 1: Drop fires for fn param at function end.
///
/// `fn consume(_t: Tracker) { }` — _t goes out of scope at end of consume().
/// elaborate_drops inserts Drop terminator there, drop glue calls user's
/// Drop::drop, counter increments. Caller observes counter = 1.
#[test]
fn stage30_3_positive_drop_fires_for_param() {
    assert_runtime(
        "drop-fires-for-param",
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn consume(_t: Tracker) {
    // _t goes out of scope at end of consume
}
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    let t = Tracker { count_ptr: counter };
    consume(t);
    println!("{}", *counter);
    0
}
"#,
        "1\n",
    );
}

/// Stage 30.3 positive 2: Drop fires for top-level local before program exit.
///
/// In this test, the drop side effect is the LAST thing before return —
/// so even though StorageDead is at function end, the drop fires before
/// the function returns. We verify via the counter value.
#[test]
fn stage30_3_positive_drop_fires_at_fn_end() {
    assert_runtime(
        "drop-fires-at-fn-end",
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn make_and_drop(counter: *mut i32) -> i32 {
    let _t = Tracker { count_ptr: counter };
    // _t goes out of scope at end of make_and_drop
    42
}
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    let _ = make_and_drop(counter);
    println!("{}", *counter);
    0
}
"#,
        "1\n",
    );
}

/// Stage 30.3 positive 3: Drop glue IS emitted for types with `impl Drop`.
///
/// This is a compile-time check (not runtime) — verifies the compiler
/// accepts `impl Drop` and produces no codegen errors. Runtime drop timing
/// is documented in the negative tests below.
#[test]
fn stage30_3_positive_drop_glue_emitted() {
    let result = compile(
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    let _t = Tracker { count_ptr: counter };
    0
}
"#,
    );
    assert_eq!(result.errors.typeck.len(), 0, "typeck should pass");
    assert_eq!(result.errors.codegen.len(), 0, "codegen should pass");
}

/// Stage 30.3 positive 4: Drop on moved value fires at destination's scope end.
#[test]
fn stage30_3_positive_drop_on_moved_value() {
    assert_runtime(
        "drop-on-moved-value",
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn take_ownership(_t: Tracker) {
    // _t dropped at end of take_ownership
}
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    let t = Tracker { count_ptr: counter };
    take_ownership(t);  // t moved into take_ownership
    // After call, t is no longer valid — drop fires in take_ownership
    println!("{}", *counter);
    0
}
"#,
        "1\n",
    );
}

// ============================================================================
// NEGATIVE TESTS — Known limitation: drop does NOT fire at block scope end
// (documented as TD-DROP-SCOPE-TIMING, P2, deferred to v0.14+)
// ============================================================================

/// Stage 30.3 negative 1 → Stage 30.6 FIXED: Drop DOES fire at block scope end.
///
/// Stage 30.6 (v0.14 TD-DROP-SCOPE-TIMING) FIX:
/// `StorageDead` is now emitted at block scope end (via scope tracking
/// in MirLowerCtxt), not at function end. So the drop fires BEFORE
/// the println, and the counter is 1 when printed.
///
/// This test was previously a KNOWN LIMITATION (Stage 30.3) that
/// documented the old behavior (drop too late). Now it verifies the
/// fix works correctly.
#[test]
fn stage30_3_negative_drop_does_not_fire_at_block_scope_end() {
    let (stdout, _exit) = run_program(
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    {
        let _t = Tracker { count_ptr: counter };
        // _t goes out of scope HERE — StorageDead emitted at block end
    }
    // _t's drop HAS fired — counter is 1
    println!("{}", *counter);
    0
}
"#,
    );
    // Stage 30.6 FIX: drop now fires at block scope end → counter = 1
    assert_eq!(
        stdout, "1\n",
        "Stage 30.6 FIX: drop should fire at block scope end (counter=1). \
         If this fails with '0', TD-DROP-SCOPE-TIMING regressed."
    );
}

/// Stage 30.3 negative 2 → Stage 30.6 FIXED: Drop DOES fire at if-block end.
#[test]
fn stage30_3_negative_drop_does_not_fire_at_if_block_end() {
    let (stdout, _exit) = run_program(
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    if true {
        let _t = Tracker { count_ptr: counter };
        // _t drops at end of if-block
    }
    // _t's drop HAS fired — counter is 1
    println!("{}", *counter);
    0
}
"#,
    );
    // Stage 30.6 FIX: drop now fires at if-block scope end → counter = 1
    assert_eq!(
        stdout, "1\n",
        "Stage 30.6 FIX: drop should fire at if-block scope end (counter=1)."
    );
}

/// Stage 30.3 negative 3 → Stage 30.6 FIXED: Drop DOES fire at loop iteration end.
#[test]
fn stage30_3_negative_drop_does_not_fire_at_loop_iteration_end() {
    let (stdout, _exit) = run_program(
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    let mut i = 0;
    while i < 3 {
        let _t = Tracker { count_ptr: counter };
        // _t drops at end of each iteration
        i = i + 1;
    }
    // After 3 iterations, counter should be 3 (drop fired each iteration)
    println!("{}", *counter);
    0
}
"#,
    );
    // Stage 30.6 FIX: drop now fires at loop iteration end → counter = 3
    assert_eq!(
        stdout, "3\n",
        "Stage 30.6 FIX: drop should fire at each loop iteration end (counter=3)."
    );
}

// ============================================================================
// REGRESSION TESTS — Drop glue emission verification
// ============================================================================

/// Stage 30.3 regression 1: Compile-time verification that Drop trait is accepted.
#[test]
fn stage30_3_regression_drop_trait_accepted() {
    let result = compile(
        r#"
struct S { x: i32 }
impl Drop for S {
    fn drop(&mut self) {
        self.x = 0;
    }
}
fn main() {
    let _s = S { x: 42 };
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Drop trait should compile cleanly"
    );
    assert_eq!(
        result.errors.codegen.len(),
        0,
        "Drop codegen should produce no errors"
    );
}

/// Stage 30.3 regression 2: Nested drop — struct without Drop containing
/// struct with Drop. The outer struct should trigger inner's drop glue.
#[test]
fn stage30_3_regression_nested_drop_compiles() {
    let result = compile(
        r#"
struct Inner { x: i32 }
impl Drop for Inner {
    fn drop(&mut self) {
        self.x = 0;
    }
}
struct Outer { inner: Inner }
fn main() {
    let _o = Outer { inner: Inner { x: 1 } };
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Nested drop should compile cleanly"
    );
    assert_eq!(
        result.errors.codegen.len(),
        0,
        "Nested drop codegen should produce no errors"
    );
}
