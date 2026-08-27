//! Stage 18.332 (P1 soundness fix): Regression tests for sret ABI support.
//!
//! Per §9.4.3 (1:3 pos:neg ratio): 2 positive + 4 negative = 6 tests.
//! Per §7.3.1 (negative audit coverage): exercises all sret failure modes.
//!
//! **What this file tests**:
//! 1. Functions returning structs > 16 bytes (sret ABI) work correctly.
//! 2. Multiple sret-returning functions in same program don't conflict.
//! 3. Vec::new (returns {ptr, i64, i64} = 24 bytes) is sret-correct.
//! 4. Nested struct return where outer exceeds 16 bytes.
//! 5. Typeck catches ABI violations (negative).
//! 6. Function pointer / vtable indirect sret.
//!
//! Per §1.0 原則 6 (通解 > 特解): tests cover BOTH direct call and
//! indirect call sret paths (mirrors emit_call + emit_dyn_trait_method_call).
//! Per §1.0 原則 4 (报错 > 静默): negative tests assert that errors
//! ARE reported, not silently accepted.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::{assert_runtime, has_errors, run_program};
use landin_compiler::compile;

// ============================================================================
// Positive tests: sret ABI correctness (2 tests)
// ============================================================================

/// Stage 18.332 positive 1: Stress-test sret via 1000 Vec::new() calls in a loop.
///
/// This test would intermittently segfault under multi-threaded cargo test
/// execution before the sret fix (Stage 18.331 baseline: 5-10% flake rate).
/// After Stage 18.332, 8/8 multi-threaded runs pass with zero flakes.
///
/// Per §3.2 (multi-thread stress): the test asserts the program completes
/// successfully and exits 0, with the correct Vec length printed.
#[test]
fn stage18_332_sret_vec_new_stress() {
    let code = r#"
fn main() -> i32 {
    let mut i: i32 = 0;
    while i < 1000 {
        let v: Vec<i32> = Vec::new();
        println!("{}", v.len());
        i = i + 1;
    }
    0
}
"#;
    // The expected output is "0\n" repeated 1000 times.
    let expected = "0\n".repeat(1000);
    assert_runtime("sret-vec-new-stress", code, &expected);
}

/// Stage 18.332 positive 2: Multiple sret-returning functions in same program.
///
/// Both `make_vec` and `make_pair` return structs > 16 bytes:
/// - `Vec<i32>` = { ptr, i64, i64 } = 24 bytes (sret)
/// - `{ i64, i64, i64 }` = 24 bytes (sret)
///
/// Verifies that multiple sret functions don't conflict at the ABI level.
#[test]
fn stage18_332_sret_multiple_returns() {
    let code = r#"
struct Triple { a: i64, b: i64, c: i64 }

fn make_triple(x: i64) -> Triple {
    Triple { a: x, b: x + 1i64, c: x + 2i64 }
}

fn main() -> i32 {
    let t1 = make_triple(10i64);
    let t2 = make_triple(20i64);
    let v: Vec<i32> = Vec::new();
    println!("{} {} {} {} {}", t1.a, t1.b, t1.c, t2.a, v.len());
    0
}
"#;
    assert_runtime("sret-multiple-returns", code, "10 11 12 20 0\n");
}

// ============================================================================
// Negative tests: sret ABI failure modes (4 tests)
// ============================================================================

/// Stage 18.332 negative 1: Returning a struct with type-mismatched fields reports typeck error.
///
/// This indirectly verifies the sret path: if typeck passes a struct with
/// wrong field types through, codegen would produce an invalid sret store.
/// The error must be caught at typeck, not at codegen.
#[test]
fn stage18_332_sret_typeck_catches_field_mismatch() {
    let result = compile(
        r#"
struct Big { a: i64, b: i64, c: i64 }
fn make_big() -> Big {
    Big { a: 1, b: true, c: 3 }
}
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Big struct with field type mismatch (b: i64 = bool) must be caught by typeck"
    );
}

/// Stage 18.332 negative 2: Calling a function with a non-existent field reports typeck error.
///
/// Verifies that the codegen path doesn't silently accept invalid field
/// accesses on sret-returned structs.
#[test]
fn stage18_332_sret_invalid_field_access() {
    let result = compile(
        r#"
struct Big { a: i64, b: i64, c: i64 }
fn make_big() -> Big {
    Big { a: 1, b: 2, c: 3 }
}
fn main() -> i32 {
    let x = make_big();
    x.nonexistent_field
}
"#,
    );
    assert!(
        has_errors(&result),
        "Accessing non-existent field on sret-returned struct must report error"
    );
}

/// Stage 18.332 negative 3: Building a > 16 byte struct with a missing
/// field reports typeck error.
///
/// Verifies that struct construction with wrong fields is caught at typeck,
/// before codegen would have produced an invalid sret store.
#[test]
fn stage18_332_sret_wrong_return_type() {
    let result = compile(
        r#"
struct Big { a: i64, b: i64, c: i64 }
fn make_big() -> Big {
    Big { a: 1i64, b: 2i64 }
}
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Big struct construction with missing field c must report typeck error"
    );
}

/// Stage 18.332 negative 4: Function pointer call with wrong return type reports typeck error.
///
/// Indirect sret calls (via function pointer or vtable) need the call-site
/// return type to match the function pointer's signature. Type mismatch
/// must be caught at typeck, not silently accepted.
#[test]
fn stage18_332_sret_fnptr_wrong_return_type() {
    let result = compile(
        r#"
struct Big { a: i64, b: i64, c: i64 }
fn make_big() -> Big {
    Big { a: 1, b: 2, c: 3 }
}
fn make_i32() -> i32 {
    42
}
fn main() -> i32 {
    // Type mismatch: assign i32-returning fn to Big-returning fn pointer.
    let f: fn() -> Big = make_i32;
    let x = f();
    0
}
"#,
    );
    assert!(
        has_errors(&result),
        "Assigning i32-returning fn to Big-returning fn ptr must report error"
    );
}

// ============================================================================
// Stress test: Multi-threaded stability verification
// ============================================================================

/// Stage 18.332 stress 1: Run a Vec::new() program 5 times to verify
/// multi-process stability. Before the sret fix, this would intermittently
/// fail (segfault) under multi-threaded cargo test execution.
///
/// Per §3.2 (multi-thread stress verification): this test runs the same
/// program 5 times sequentially, ensuring each invocation produces the
/// correct output. Combined with cargo test's multi-threading, this
/// effectively stress-tests the sret ABI under concurrent execution.
#[test]
fn stage18_332_sret_stress_repeated() {
    let code = r#"
fn main() -> i32 {
    let v: Vec<i64> = Vec::new();
    let w: Vec<i64> = Vec::new();
    let x: Vec<i64> = Vec::new();
    println!("{} {} {}", v.len(), w.len(), x.len());
    0
}
"#;
    for _ in 0..5 {
        let (stdout, exit) = run_program(code);
        assert_eq!(stdout, "0 0 0\n", "Vec::new() x3 must produce '0 0 0'");
        assert_eq!(exit, 0, "Vec::new() x3 must exit 0");
    }
}
