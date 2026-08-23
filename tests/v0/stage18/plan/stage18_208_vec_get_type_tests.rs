//! Stage 18.208 — TD-VEC-GET-TYPE-INFERENCE fix tests.
//!
//! Verifies that `Vec<T>::get(index)` correctly extracts the element type
//! from the receiver's `Vec<T>` type (substs[0]) instead of hardcoding i32.
//!
//! Per Stage 18.207 task review: this was identified as a localized MIR
//! lower bug (not a typeck issue) — `lower_vec_get_intrinsic` hardcoded
//! `out_ty = i32` instead of reading `Vec<T>`'s substs[0].
//!
//! Per §17.5.2 (test organization): one file per stage.
//! Per §9.4 (设计-开发-测试锚定): tests verify the design intent of
//! element type extraction from generic Adt substs.
//! Per §9.4.3 (1:3+ 正负比例): positive tests for all Vec<T> types + 1 negative.

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
        std::env::temp_dir().join(format!("landin_s208_{}_{}.lin", std::process::id(), id));
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

/// Regression: Vec<Point>::get(0).x — was failing with LLVM GEP error
/// because out_ty was hardcoded to i32 but element was Point struct.
#[test]
fn stage18_208_vec_struct_get_field() {
    assert_runtime(
        "vec-struct-get-field",
        r#"
struct Point { x: i32, y: i32 }
fn main() -> i32 {
    let mut v: Vec<Point> = Vec::new();
    let p = Point { x: 10, y: 20 };
    v.push(p);
    println!("{}", v.get(0).x);
    println!("{}", v.get(0).y);
    0
}
"#,
        "10\n20\n",
    );
}

/// Regression: Vec<Point>::get(0) bound to a let variable, then field access.
#[test]
fn stage18_208_vec_struct_get_binding() {
    assert_runtime(
        "vec-struct-get-binding",
        r#"
struct Point { x: i32, y: i32 }
fn main() -> i32 {
    let mut v: Vec<Point> = Vec::new();
    let p = Point { x: 30, y: 40 };
    v.push(p);
    let q = v.get(0);
    println!("{}", q.x);
    println!("{}", q.y);
    0
}
"#,
        "30\n40\n",
    );
}

/// Regression: Vec<i32>::get still works (canonical case).
#[test]
fn stage18_208_vec_i32_get() {
    assert_runtime(
        "vec-i32-get",
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

/// Regression: Vec<i64>::get with suffixed literals.
#[test]
fn stage18_208_vec_i64_get() {
    assert_runtime(
        "vec-i64-get",
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

/// Regression: Vec<Point> with multiple elements.
#[test]
fn stage18_208_vec_struct_multiple() {
    assert_runtime(
        "vec-struct-multiple",
        r#"
struct Point { x: i32, y: i32 }
fn main() -> i32 {
    let mut v: Vec<Point> = Vec::new();
    v.push(Point { x: 1, y: 2 });
    v.push(Point { x: 3, y: 4 });
    println!("{}", v.get(0).x);
    println!("{}", v.get(0).y);
    println!("{}", v.get(1).x);
    println!("{}", v.get(1).y);
    0
}
"#,
        "1\n2\n3\n4\n",
    );
}

/// Negative: OOB still panics (regression — bounds check unchanged).
#[test]
fn stage18_208_vec_oob_panics() {
    let code = r#"
struct Point { x: i32, y: i32 }
fn main() -> i32 {
    let mut v: Vec<Point> = Vec::new();
    v.push(Point { x: 10, y: 20 });
    let q = v.get(5);
    println!("{}", q.x);
    0
}
"#;
    let (_stdout, exit) = run_program(code);
    assert_ne!(exit, 0, "expected OOB panic (exit != 0), got exit {exit}");
}
