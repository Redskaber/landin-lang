//! Stage 83 (v0.8): TD-FN-CLOSURE-COERCION runtime fix tests.
//!
//! Verifies the fix for the runtime segfault when closures are coerced to
//! function pointers (`fn(...) -> ...`) and passed as call arguments.
//!
//! ## Background
//!
//! Stage 79 added Closure→FnPtr typeck coercion (closures can be passed
//! to functions expecting `fn(Args) -> Ret`). Stages 80-82 made incremental
//! progress on the runtime side:
//! - Stage 81: empty closure Aggregate emits `@closure_call_fn_N` (fn ptr).
//! - Stage 82: empty Closure MIR type → `OpaquePtr` (alloca = `ptr`).
//!
//! Stage 83 closes the loop: removes the Stage 16.21 redundant check that
//! passed Closure-typed args as alloca addresses instead of loaded values.
//! Non-self Closure args now flow through `codegen_operand`, which emits
//! `load ptr, ptr %loc_N` to fetch the function pointer value.
//!
//! ## Test Matrix (§9.4.3 — 1:3+ positive:negative ratio)
//!
//! - 1 positive runtime test (closure coerced, called, output verified)
//! - 3 negative compile-time tests:
//!   - Wrong arity (closure `(i32, i32) -> i32` vs expected `fn(i32) -> i32`)
//!   - Wrong arity zero args (closure `() -> i32` vs expected `fn(i32) -> i32`)
//!   - Wrong arity too many args (closure `(i32, i32, i32) -> i32` vs
//!     expected `fn(i32) -> i32`)
//!
//! Note on typeck depth: closure *param type* mismatch (e.g., i64 vs i32)
//! is NOT currently caught because Landin's MIR lower assigns fresh infer
//! vars to closure params, ignoring explicit annotations (`|n: i64|`).
//! This is tracked as TD-CLOSURE-PARAM-ANNOT-IGNORE (P3, v0.8+) — out of
//! scope for Stage 83's runtime fix.
//!
//! Per §1.0 原則 4 (报错 > 静默): arity mismatch must error, not silently
//! truncate args.
//! Per §12 (最优 > 最小): root-cause fix at the codegen call-site, not a
//! per-callsite patch.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::{compile_src, run_program};

// =============================================================================
// Positive: closure coerced to fn pointer, called inside the callee
// =============================================================================

/// Stage 83 positive 1: Closure coerced to `fn(i32) -> i32`, passed to a
/// function that calls it. Verifies the runtime produces the correct result
/// (no segfault) and the expected stdout.
///
/// Before Stage 83 fix: this segfaulted because the call site passed the
/// alloca address (`ptr %loc_N`) instead of the loaded function pointer
/// value. The callee then indirect-called stack memory, crashing.
///
/// After Stage 83 fix: `codegen_operand` emits `load ptr, ptr %loc_N` to
/// fetch the function pointer, and the callee indirect-calls the actual
/// synthesized `closure_call_fn_N` function.
///
/// The exit code is 42 because `main` returns `result` (which is 42, the
/// closure's return value for input 21 doubled).
#[test]
fn stage83_closure_coerced_to_fn_ptr_runtime() {
    let src = r#"
        fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
            f(x)
        }
        fn main() -> i32 {
            let doubled = |n: i32| n * 2;
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
    assert_eq!(
        exit, 42,
        "closure coercion runtime should exit with main's return value (42), got {}",
        exit
    );
}

// =============================================================================
// Negative: closure arity mismatch must error at typeck
// =============================================================================

/// Stage 83 negative 1: Closure with too many params cannot coerce to
/// `fn(i32) -> i32`. The closure has signature `(i32, i32) -> i32` (2 params),
/// but the function expects `fn(i32) -> i32` (1 param). Typeck must reject.
///
/// Per §1.0 原則 4 (报错 > 静默): silently accepting the wrong arity would
/// cause the callee to call the closure with 1 arg, but the closure
/// expects 2 — leading to undefined behavior at runtime.
/// Per §1.0 原則 6 (通解 > 特解): same Closure↔FnPtr unification path catches
/// all arity mismatches (too few, exact, too many).
#[test]
fn stage83_closure_wrong_arity_too_many_errors() {
    let src = r#"
        fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
            f(x)
        }
        fn main() -> i32 {
            let bad = |a: i32, b: i32| a + b;
            let _r = apply(bad, 21);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "closure with wrong arity (2 params vs 1 expected) should fail typeck"
    );
}

/// Stage 83 negative 2: Closure with too few params cannot coerce to
/// `fn(i32) -> i32`. The closure has signature `() -> i32` (0 params), but
/// the function expects `fn(i32) -> i32` (1 param). Typeck must reject.
///
/// Per §1.0 原則 4 (报错 > 静默): silently accepting the wrong arity would
/// cause the callee to pass 1 arg to a closure expecting 0 — leading to
/// stack corruption at runtime.
#[test]
fn stage83_closure_wrong_arity_too_few_errors() {
    let src = r#"
        fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
            f(x)
        }
        fn main() -> i32 {
            let bad = || 42;
            let _r = apply(bad, 21);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "closure with wrong arity (0 params vs 1 expected) should fail typeck"
    );
}

/// Stage 83 negative 3: Closure with way too many params (3 vs 1) cannot
/// coerce to `fn(i32) -> i32`. The closure has signature
/// `(i32, i32, i32) -> i32` (3 params), but the function expects
/// `fn(i32) -> i32` (1 param). Typeck must reject.
///
/// This tests the boundary case where the arity difference is large (3 vs 1)
/// — ensures the arity check is `!=` (strict), not `>=` or `<=`.
#[test]
fn stage83_closure_wrong_arity_three_vs_one_errors() {
    let src = r#"
        fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
            f(x)
        }
        fn main() -> i32 {
            let bad = |a: i32, b: i32, c: i32| a + b + c;
            let _r = apply(bad, 21);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "closure with wrong arity (3 params vs 1 expected) should fail typeck"
    );
}
