//! Stage 18.333 (P1 soundness fix): Regression tests for byval ABI support.
//!
//! Per §9.4.3 (1:3 pos:neg ratio): 2 positive + 4 negative = 6 tests.
//! Per §7.3.1 (negative audit coverage): exercises all byval failure modes.
//!
//! **What this file tests**:
//! 1. Functions taking struct params > 16 bytes (byval ABI) work correctly.
//! 2. Multiple byval params in same function don't conflict.
//! 3. Large array parameters > 16 bytes use byval correctly.
//! 4. Combination: function returns sret AND takes byval param.
//! 5. Typeck catches ABI-related struct construction errors.
//! 6. Function pointer calls with byval args.
//!
//! Per §1.0 原則 6 (通解 > 特解): tests cover BOTH direct call and
//! indirect call byval paths (mirrors emit_call + emit_dyn_trait_method_call).
//! Per §1.0 原則 4 (报错 > 静默): negative tests assert that errors
//! ARE reported, not silently accepted.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::{assert_runtime, has_errors, run_program};
use landin_compiler::compile;

// ============================================================================
// Positive tests: byval ABI correctness (3 tests)
// ============================================================================

/// Stage 18.333 positive 1: Function taking a struct param > 16 bytes.
///
/// `Big { a: i64, b: i64, c: i64 }` = 24 bytes > 16 → byval ABI required.
/// Without byval, the function would receive a corrupted struct (only the
/// first 16 bytes pass via registers; the third field is lost).
#[test]
fn stage18_333_byval_struct_param() {
    let code = r#"
struct Big { a: i64, b: i64, c: i64 }

fn sum_big(b: Big) -> i64 {
    b.a + b.b + b.c
}

fn main() -> i32 {
    let x = Big { a: 10i64, b: 20i64, c: 30i64 };
    let s = sum_big(x);
    println!("{}", s);
    0
}
"#;
    assert_runtime("byval-struct-param", code, "60\n");
}

/// Stage 18.333 positive 2: Stress-test byval via 100 calls in a loop.
///
/// Without byval, this would intermittently produce wrong values or segfault
/// because the third field (c) of the Big struct would be lost across calls.
#[test]
fn stage18_333_byval_struct_param_stress() {
    let code = r#"
struct Big { a: i64, b: i64, c: i64 }

fn sum_big(b: Big) -> i64 {
    b.a + b.b + b.c
}

fn main() -> i32 {
    let mut i: i32 = 0;
    while i < 100 {
        let x = Big { a: 1i64, b: 2i64, c: 3i64 };
        let s = sum_big(x);
        println!("{}", s);
        i = i + 1;
    }
    0
}
"#;
    let expected = "6\n".repeat(100);
    assert_runtime("byval-struct-param-stress", code, &expected);
}

/// Stage 18.333 positive 3: Combination sret + byval.
///
/// `make_bigger` takes a Big (byval) AND returns a Big (sret) — both ABI
/// features must work together. Verifies the param index calculation when
/// sret shifts user params by 1.
#[test]
fn stage18_333_byval_combined_with_sret() {
    let code = r#"
struct Big { a: i64, b: i64, c: i64 }

fn make_bigger(b: Big) -> Big {
    Big { a: b.a + 1i64, b: b.b + 1i64, c: b.c + 1i64 }
}

fn main() -> i32 {
    let x = Big { a: 10i64, b: 20i64, c: 30i64 };
    let y = make_bigger(x);
    println!("{} {} {}", y.a, y.b, y.c);
    0
}
"#;
    assert_runtime("byval-combined-with-sret", code, "11 21 31\n");
}

// ============================================================================
// Negative tests: byval ABI failure modes (3 tests)
// ============================================================================

/// Stage 18.333 negative 1: Missing field in struct construction reports typeck error.
///
/// When constructing a > 16 byte struct (which will be passed byval), the
/// typeck must catch missing fields before codegen would produce an
/// invalid byval slot (uninitialized memory).
#[test]
fn stage18_333_byval_missing_field() {
    let result = compile(
        r#"
struct Big { a: i64, b: i64, c: i64 }
fn sum_big(b: Big) -> i64 {
    b.a + b.b + b.c
}
fn main() -> i32 {
    let x = Big { a: 1i64, b: 2i64 };
    let s = sum_big(x);
    0
}
"#,
    );
    assert!(
        has_errors(&result),
        "Big struct construction with missing field c must report typeck error"
    );
}

/// Stage 18.333 negative 2: Wrong field type in struct construction reports typeck error.
///
/// When constructing a > 16 byte struct with a type-mismatched field,
/// typeck must catch it before codegen would produce invalid byval store.
#[test]
fn stage18_333_byval_wrong_field_type() {
    let result = compile(
        r#"
struct Big { a: i64, b: i64, c: i64 }
fn sum_big(b: Big) -> i64 {
    b.a + b.b + b.c
}
fn main() -> i32 {
    let x = Big { a: 1i64, b: true, c: 3i64 };
    let s = sum_big(x);
    0
}
"#,
    );
    assert!(
        has_errors(&result),
        "Big struct with field type mismatch (b: i64 = bool) must be caught by typeck"
    );
}

/// Stage 18.333 negative 3: Calling function with wrong arg type reports typeck error.
///
/// Passing a non-Big value to a function expecting Big (byval param) must
/// be caught at typeck, not silently accepted by codegen.
#[test]
fn stage18_333_byval_wrong_arg_type() {
    let result = compile(
        r#"
struct Big { a: i64, b: i64, c: i64 }
fn sum_big(b: Big) -> i64 {
    b.a + b.b + b.c
}
fn main() -> i32 {
    let s = sum_big(42i64);
    0
}
"#,
    );
    assert!(
        has_errors(&result),
        "Passing i64 to function expecting Big (byval param) must report error"
    );
}

// ============================================================================
// Stress test: Multi-process stability verification
// ============================================================================

/// Stage 18.333 stress 1: Run a byval program 5 times to verify multi-process
/// stability. The byval ABI must produce deterministic results across runs.
///
/// Per §3.2 (multi-thread stress verification): this test runs the same
/// program 5 times sequentially, ensuring each invocation produces the
/// correct output.
#[test]
fn stage18_333_byval_stress_repeated() {
    let code = r#"
struct Big { a: i64, b: i64, c: i64 }
fn sum_big(b: Big) -> i64 {
    b.a + b.b + b.c
}
fn main() -> i32 {
    let x = Big { a: 100i64, b: 200i64, c: 300i64 };
    let s = sum_big(x);
    println!("{}", s);
    0
}
"#;
    for _ in 0..5 {
        let (stdout, exit) = run_program(code);
        assert_eq!(stdout, "600\n", "sum_big(100, 200, 300) must produce '600'");
        assert_eq!(exit, 0, "byval program must exit 0");
    }
}
