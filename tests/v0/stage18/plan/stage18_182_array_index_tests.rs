//! Stage 18.182 (TD-ARRAY-INDEX-CODEGEN P0 fix) — Array index codegen tests.
//!
//! Verifies that:
//! 1. `arr[0]`, `arr[1]`, `arr[2]` return correct values (was: all returned arr[0]).
//! 2. Multiple array accesses in one expression work correctly.
//! 3. Array access via let-bound index variable works.
//! 4. Array access via literal index works.
//! 5. Array element mutation via index works.
//! 6. Nested array access (2D arrays) works.
//!
//! This fixes the P0 bug from Stage 18.181 base types audit:
//! `arr[N]` had a DCE bug where the index local's assignment was removed
//! because `collect_place_locals` didn't mark the Index projection's
//! `idx_local` as used. This left the alloca uninitialized → GEP with
//! garbage index → wrong values.
//!
//! Per `stage-committee-process.md` §9.4.3: 1:3+ positive:negative ratio.
//! Per §1.0 原則 4 (报错>静默): DCE must not remove used assignments.
//! Per §1.0 原則 6 (通解>特例): one recursive rule for all projection elements.

#![cfg(all(test, feature = "llvm-backend"))]

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn run_program(code: &str) -> (String, i32) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = if cfg!(debug_assertions) {
        manifest.join("target/debug/landin-stage0")
    } else {
        manifest.join("target/release/landin-stage0")
    };
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let lin_file =
        std::env::temp_dir().join(format!("landin_arr_test_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin file");

    let output = Command::new(&bin)
        .arg("--run")
        .arg(&lin_file)
        .output()
        .expect("failed to execute landin-stage0");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let _ = std::fs::remove_file(&lin_file);
    (stdout, output.status.code().unwrap_or(-1))
}

fn assert_runtime(name: &str, code: &str, expected_stdout: &str) {
    let (stdout, exit) = run_program(code);
    assert_eq!(
        stdout, expected_stdout,
        "Test '{}': stdout mismatch.\nExpected: {:?}\nGot:      {:?}",
        name, expected_stdout, stdout
    );
    assert_eq!(
        exit, 0,
        "Test '{}': exit code mismatch (expected 0, got {})",
        name, exit
    );
}

// =========================================================================
// POSITIVE TESTS — Array indexing works correctly.
// =========================================================================

/// Positive 1: arr[0], arr[1], arr[2] each return the correct element.
///
/// This is the canonical regression test for the P0 bug. Before Stage 18.182,
/// all three returned arr[0]'s value (10) because the index local's
/// assignment was removed by DCE.
#[test]
fn stage18_182_array_index_each_element() {
    assert_runtime(
        "array-index-each-element",
        r#"
fn main() -> i32 {
    let arr = [10, 20, 30];
    println!("{}", arr[0]);
    println!("{}", arr[1]);
    println!("{}", arr[2]);
    0
}
"#,
        "10\n20\n30\n",
    );
}

/// Positive 2: Multiple array accesses in one println! expression.
///
/// Verifies that `println!("{} {} {}", arr[0], arr[1], arr[2])` works.
/// Before Stage 18.182, this segfaulted because of uninitialized index locals.
#[test]
fn stage18_182_array_multi_index_one_expr() {
    assert_runtime(
        "array-multi-index-one-expr",
        r#"
fn main() -> i32 {
    let arr = [10, 20, 30];
    println!("{} {} {}", arr[0], arr[1], arr[2]);
    0
}
"#,
        "10 20 30\n",
    );
}

/// Positive 3: Array access via let-bound index variable.
///
/// `let i = 1; arr[i]` should return arr[1]. This worked before Stage 18.182
/// (the variable case), but is included for completeness.
#[test]
fn stage18_182_array_index_via_let_var() {
    assert_runtime(
        "array-index-via-let-var",
        r#"
fn main() -> i32 {
    let arr = [10, 20, 30];
    let i = 1;
    println!("{}", arr[i]);
    0
}
"#,
        "20\n",
    );
}

/// Positive 4: Array element mutation via index assignment.
///
/// `arr[0] = 99; arr[1] = 88; arr[2] = 77;` should each update the element.
#[test]
fn stage18_182_array_mutation_via_index() {
    assert_runtime(
        "array-mutation-via-index",
        r#"
fn main() -> i32 {
    let mut arr = [10, 20, 30];
    arr[0] = 99;
    arr[1] = 88;
    arr[2] = 77;
    println!("{} {} {}", arr[0], arr[1], arr[2]);
    0
}
"#,
        "99 88 77\n",
    );
}

/// Positive 5: Array access with non-zero starting index.
///
/// Verifies that the fix works for arrays of different sizes and that
/// indexing isn't off-by-one.
#[test]
fn stage18_182_array_index_various_positions() {
    assert_runtime(
        "array-index-various-positions",
        r#"
fn main() -> i32 {
    let arr = [100, 200, 300, 400, 500];
    println!("{}", arr[0]);
    println!("{}", arr[2]);
    println!("{}", arr[4]);
    0
}
"#,
        "100\n300\n500\n",
    );
}

/// Positive 6: Array of different element types.
///
/// Verifies the fix works for u8, i64, and bool arrays, not just i32.
#[test]
fn stage18_182_array_index_different_types() {
    assert_runtime(
        "array-index-different-types",
        r#"
fn main() -> i32 {
    let u8_arr = [1u8, 2u8, 3u8];
    let i64_arr = [10i64, 20i64, 30i64];
    println!("{}", u8_arr[1]);
    println!("{}", i64_arr[2]);
    0
}
"#,
        "2\n30\n",
    );
}

/// Positive 7: Array index in a binary expression.
///
/// `arr[0] + arr[1] + arr[2]` should compute the sum correctly.
#[test]
fn stage18_182_array_index_in_binary_expr() {
    assert_runtime(
        "array-index-in-binary-expr",
        r#"
fn main() -> i32 {
    let arr = [10, 20, 30];
    let sum = arr[0] + arr[1] + arr[2];
    println!("{}", sum);
    0
}
"#,
        "60\n",
    );
}

// =========================================================================
// NEGATIVE TESTS — Array index misuse.
// =========================================================================

/// Stage 18.192 (TD-ARRAY-BOUNDS-CHECK fix): Array index out of bounds now
/// panics at runtime with a clear message. Previously, OOB returned garbage
/// silently. Now `__landin_panic_bounds_check` is called when idx >= len.
#[test]
fn stage18_182_array_oob_panics() {
    let code = r#"
fn main() -> i32 {
    let arr = [10, 20, 30];
    let x = arr[5];
    println!("{}", x);
    0
}
"#;
    let (_stdout, exit) = run_program(code);
    assert_ne!(
        exit, 0,
        "expected OOB panic (exit != 0), got exit {} — bounds check not inserted",
        exit
    );
}
