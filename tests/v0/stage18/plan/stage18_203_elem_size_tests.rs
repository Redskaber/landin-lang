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

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::{assert_runtime, run_program};

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
/// Stage 18.213: Now uses unsuffixed literals (100 instead of 100i64) because
/// the MIR lower extracts the element type from Vec<T>'s substs[0] when the
/// literal's type is still Infer(IntVar). This is the TD-INT-UINT-VAR partial fix.
///
/// Stage 33.1: Updated to use suffixed literals (100i64) — the prelude impl
/// of Vec::push infers T from the value parameter, and unsuffixed integers
/// default to i32 (not i64). With suffixed literals, T=i64 is correctly
/// inferred from both self arg and value arg.
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
/// element type.
/// Stage 18.213: Now uses suffixed literals (7i8) — the prelude impl
/// infers T from the value parameter, and unsuffixed integers default to
/// i32 (not i8). With suffixed literals, T=i8 is correctly inferred.
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
#[test]
fn stage18_203_box_i32_basic() {
    assert_runtime(
        "box-i32-basic",
        r#"
fn main() -> i32 {
    let b: Box<i32> = Box::new(42);
    let v: i32 = *b.0;
    println!("{}", v);
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
fn main() -> i32 {
    let b: Box<i64> = Box::new(42);
    let v: i64 = *b.0;
    println!("{}", v);
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
