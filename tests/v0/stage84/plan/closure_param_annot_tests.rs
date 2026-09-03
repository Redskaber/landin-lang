//! Stage 84 (v0.8): TD-CLOSURE-PARAM-ANNOT-IGNORE FIXED.
//!
//! Verifies that explicit type annotations on closure params are now
//! respected by MIR lower AND typeck. Previously, all closure params got
//! fresh infer var types — ignoring user-supplied annotations like
//! `|n: i64|`. This broke Closure↔FnPtr typeck coercion: the infer var
//! unified with any concrete type, so `apply(|n: i64| ..., 21)` (where
//! apply expects `fn(i32) -> i32`) silently compiled and produced
//! runtime UB.
//!
//! ## Background
//!
//! The fix spans two locations (root cause = same dispatch logic):
//! - `src/mir/lower/expr_operand.rs:1083-1103` — outer body's closure
//!   value's local gets the user-supplied type (was: fresh infer var).
//! - `src/mir/lower/body_lower.rs:789-803` — the closure's OWN MIR body's
//!   param locals get the user-supplied type (was: fresh infer var).
//! - `src/driver/compile_inner.rs:585-617` — the fn_sig_table entry for
//!   the closure uses the lowered HIR type (was: fresh infer var).
//!
//! All three sites use the same dispatch: if `param.ty` is `Some` AND
//! its kind is NOT `Infer`, lower the HIR type to MIR type; otherwise
//! allocate a fresh infer var (preserving the original behavior for
//! unannotated closure params like `|x| ...`).
//!
//! ## Test Matrix (§9.4.3 — 1:3+ positive:negative ratio)
//!
//! - 1 positive runtime test (unannotated closure still works — verifies
//!   the fix preserves backward compat for `|n| n * 2` patterns).
//! - 3 negative typeck tests:
//!   - Wrong param type (i64 vs i32 expected)
//!   - Wrong param type (u64 vs i32 expected — signedness mismatch)
//!   - Wrong param type (bool vs i32 expected — totally incompatible)
//!
//! Per §1.0 原則 4 (报错 > 静默): mismatched types must error, not silently
//! unify via fresh infer var.
//! Per §12 (最优 > 最小): root-cause fix at the MIR lower layer, not a
//! typeck "look-through" workaround.
//! Per Rust: rustc's HIR→TyLowering uses user-supplied types when
//! present, fresh infer vars only when absent. This mirrors that contract.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::{compile_src, run_program};

// =============================================================================
// Positive: unannotated closure still works (backward compat)
// =============================================================================

/// Stage 84 positive 1: Unannotated closure param (`|n| n * 2`) continues
/// to work after the Stage 84 fix. This verifies the dispatch logic:
/// unannotated params (HirTyKind::Infer) still get a fresh infer var,
/// preserving the original type inference behavior.
///
/// Without this test, a regression that always treats params as annotated
/// (or always unannotated) would silently break either this case or the
/// negative cases below.
#[test]
fn stage84_unannotated_closure_param_still_works() {
    let src = r#"
        fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
            f(x)
        }
        fn main() -> i32 {
            let doubled = |n| n * 2;
            let result = apply(doubled, 21);
            println!("result = {}", result);
            result
        }
    "#;
    let (stdout, exit) = run_program(src);
    assert!(
        stdout.contains("result = 42"),
        "expected 'result = 42' in stdout, got: {}",
        stdout
    );
    assert_eq!(exit, 42, "main should return 42, got {}", exit);
}

// =============================================================================
// Negative: closure param type mismatch must error at typeck
// =============================================================================

/// Stage 84 negative 1: Closure param annotated as `i64` cannot coerce to
/// `fn(i32) -> i32`. Typeck must report "mismatched types: expected i32,
/// found i64".
///
/// Before Stage 84 fix: this silently compiled because the closure param
/// got a fresh infer var (ignoring `: i64`), which unified with i32.
/// After Stage 84 fix: MIR lower respects the `: i64` annotation, so
/// Closure↔FnPtr unification compares i64 vs i32 and errors.
#[test]
fn stage84_closure_param_i64_vs_i32_errors() {
    let src = r#"
        fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
            f(x)
        }
        fn main() -> i32 {
            let bad = |n: i64| n as i32 * 2;
            let _r = apply(bad, 21);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "closure with `|n: i64|` passed to `fn(i32) -> i32` should fail typeck"
    );
}

/// Stage 84 negative 2: Closure param annotated as `u64` (unsigned) cannot
/// coerce to `fn(i32) -> i32` (signed). Typeck must report a mismatch.
///
/// This tests the signedness boundary — i32 and u64 differ in both width
/// AND signedness, so they must NOT unify.
///
/// Per §1.0 原則 1 (内存安全决不能妥协): silently accepting u64 as i32
/// would cause sign-extension bugs and silent data corruption.
#[test]
fn stage84_closure_param_u64_vs_i32_errors() {
    let src = r#"
        fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
            f(x)
        }
        fn main() -> i32 {
            let bad = |n: u64| n as i32 * 2;
            let _r = apply(bad, 21);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "closure with `|n: u64|` passed to `fn(i32) -> i32` should fail typeck (signedness + width mismatch)"
    );
}

/// Stage 84 negative 3: Closure param annotated as `bool` cannot coerce to
/// `fn(i32) -> i32`. Typeck must report a mismatch.
///
/// This tests a totally incompatible type (bool vs integer) — ensures
/// the Closure↔FnPtr unification catches non-numeric mismatches too,
/// not just Int-with-different-width cases.
#[test]
fn stage84_closure_param_bool_vs_i32_errors() {
    let src = r#"
        fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
            f(x)
        }
        fn main() -> i32 {
            let bad = |n: bool| if n { 1 } else { 0 };
            let _r = apply(bad, 21);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "closure with `|n: bool|` passed to `fn(i32) -> i32` should fail typeck (incompatible types)"
    );
}
