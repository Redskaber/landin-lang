//! Stage 18.203 — Unified elem_size inference tests.
//!
//! Verifies that the integrated fix for TD-BOX-SIZE-OF + TD-VEC-ELEM-SIZE-INFERENCE
//! does not regress Vec operations on i32 (canonical MVP case). The actual
//! Adt-size inference (Box::new of structs) is blocked by TD-TUPLE-CTOR-TYPECK
//! (v0.2 P2+) which prevents `Box::new(MyStruct)` from compiling — verified
//! separately via unit tests in `src/mir/lower/adt_layout.rs`.
//!
//! Per §17.5.2 (test organization): one file per stage, name = `stage-N.M-feature-tests.rs`.
//! Per §9.4 (设计-开发-测试锚定): tests verify the design intent of unified elem_size.
//! Per §9.4.3 (1:3+ 正负比例): all positive tests + at least 1 negative regression test.

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
        std::env::temp_dir().join(format!("landin_elemsize_{}_{}.lin", std::process::id(), id));
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

fn assert_runtime(name: &str, code: &str, expected: &str) {
    let (stdout, exit) = run_program(code);
    assert_eq!(
        exit, 0,
        "Test '{name}': compilation/runtime failed (exit {exit})\nstdout:\n{stdout}"
    );
    assert_eq!(
        stdout, expected,
        "Test '{name}': stdout mismatch\n  left: {stdout:?}\n  right: {expected:?}"
    );
}

/// Regression: Vec<i32> push + get roundtrip still works after Stage 18.203.
/// Verifies elem_size=4 is correctly passed to both Vec::push and Vec::get
/// (must match — mismatched elem_sizes corrupt Vec offsets).
#[test]
fn stage18_203_vec_i32_roundtrip() {
    assert_runtime(
        "vec-i32-roundtrip",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(10);
    v.push(20);
    v.push(30);
    println!("{}", v.get(0));
    println!("{}", v.get(1));
    println!("{}", v.get(2));
    0
}
"#,
        "10\n20\n30\n",
    );
}

/// Regression: Vec<i32> with growth (cap 0→4→8→16) still works.
#[test]
fn stage18_203_vec_i32_growth_roundtrip() {
    assert_runtime(
        "vec-i32-growth-roundtrip",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    v.push(4);
    v.push(5);
    println!("{}", v.get(0));
    println!("{}", v.get(4));
    0
}
"#,
        "1\n5\n",
    );
}

/// Regression: Vec<i64> (8-byte elements) roundtrip. Tests that compute_type_size
/// correctly handles i64 → 8 bytes (not 4 default fallback).
///
/// Stage 18.208: Now also tests that Vec::get correctly extracts the i64
/// element type (was hardcoded to i32, causing LLVM GEP errors).
///
/// Note: uses suffixed literals (100i64) because unsuffixed integer literals
/// default to i32 via typeck's IntVar defaulting (TD-INT-UINT-VAR, v0.2 P2+).
/// The Vec<i64> type annotation doesn't propagate to push args until typeck
/// generic instantiation is implemented (TD-TYPECK-GENERIC-INST, v0.2 P2+).
#[test]
fn stage18_203_vec_i64_roundtrip() {
    assert_runtime(
        "vec-i64-roundtrip",
        r#"
fn main() -> i32 {
    let mut v: Vec<i64> = Vec::new();
    v.push(100i64);
    v.push(200i64);
    v.push(300i64);
    println!("{}", v.get(0));
    println!("{}", v.get(1));
    println!("{}", v.get(2));
    0
}
"#,
        "100\n200\n300\n",
    );
}

/// Regression: Vec<i8> (1-byte elements) roundtrip. Tests that compute_type_size
/// correctly handles i8 → 1 byte.
///
/// Stage 18.208: Now also tests that Vec::get correctly extracts the i8
/// element type. Uses suffixed literals (7i8) because unsuffixed defaults to i32.
#[test]
fn stage18_203_vec_i8_roundtrip() {
    assert_runtime(
        "vec-i8-roundtrip",
        r#"
fn main() -> i32 {
    let mut v: Vec<i8> = Vec::new();
    v.push(7i8);
    v.push(8i8);
    v.push(9i8);
    println!("{}", v.get(0));
    println!("{}", v.get(1));
    println!("{}", v.get(2));
    0
}
"#,
        "7\n8\n9\n",
    );
}

/// Regression: Vec<u32> (4-byte elements) roundtrip.
#[test]
fn stage18_203_vec_u32_roundtrip() {
    assert_runtime(
        "vec-u32-roundtrip",
        r#"
fn main() -> i32 {
    let mut v: Vec<u32> = Vec::new();
    v.push(11);
    v.push(22);
    println!("{}", v.get(0));
    println!("{}", v.get(1));
    0
}
"#,
        "11\n22\n",
    );
}

/// Regression: Box::new(i32) still works (no segfault from elem_size change).
/// Per Stage 18.189 convention: Box is `Box(*mut T)` tuple struct; access
/// inner value via `*b.0` and explicit `__landin_dealloc` (no auto-drop yet).
#[test]
fn stage18_203_box_i32_basic() {
    assert_runtime(
        "box-i32-basic",
        r#"
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let b: Box<i32> = Box::new(42);
    let v: i32 = *b.0;
    println!("{}", v);
    __landin_dealloc(b.0 as *mut u8);
    0
}
"#,
        "42\n",
    );
}

/// Regression: Box::new(i64) — verify 8-byte allocation works.
/// Uses value 42 (matches existing Stage 18.189 test convention; larger
/// values are blocked by pre-existing TD-TUPLE-CTOR-TYPECK type-coercion issue).
#[test]
fn stage18_203_box_i64_basic() {
    assert_runtime(
        "box-i64-basic",
        r#"
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let b: Box<i64> = Box::new(42);
    let v: i64 = *b.0;
    println!("{}", v);
    __landin_dealloc(b.0 as *mut u8);
    0
}
"#,
        "42\n",
    );
}

/// Negative: OOB still panics (regression — bounds check unchanged).
#[test]
fn stage18_203_vec_oob_panics() {
    let code = r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(10);
    let x = v.get(5);
    println!("{}", x);
    0
}
"#;
    let (_stdout, exit) = run_program(code);
    assert_ne!(exit, 0, "expected OOB panic (exit != 0), got exit {exit}");
}
