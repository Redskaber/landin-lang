//! Stage 18.197 — Vec::push implementation tests.
//!
//! Verifies:
//! 1. `v.push(x)` increments len.
//! 2. Multiple pushes work.
//! 3. Vec growth (cap 0→4→8→...) works correctly.
//! 4. Different element types (i32, i64, u8) work.

#![cfg(all(test, feature = "llvm-backend"))]

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn run_program(code: &str) -> (String, i32) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/debug/landin-stage0");
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let lin_file =
        std::env::temp_dir().join(format!("landin_vecpush_{}_{}.lin", std::process::id(), id));
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
    assert_eq!(stdout, expected, "Test '{}': stdout mismatch", name);
    assert_eq!(exit, 0, "Test '{}': exit code mismatch", name);
}

#[test]
fn stage18_197_vec_push_single() {
    assert_runtime(
        "vec-push-single",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(42);
    println!("{}", v.len());
    0
}
"#,
        "1\n",
    );
}

#[test]
fn stage18_197_vec_push_multiple() {
    assert_runtime(
        "vec-push-multiple",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(10);
    v.push(20);
    v.push(30);
    println!("{}", v.len());
    0
}
"#,
        "3\n",
    );
}

#[test]
fn stage18_197_vec_push_growth() {
    assert_runtime(
        "vec-push-growth",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    v.push(4);
    v.push(5);
    println!("{} {}", v.len(), v.cap);
    0
}
"#,
        "5 8\n",
    );
}

#[test]
fn stage18_197_vec_push_i64() {
    assert_runtime(
        "vec-push-i64",
        r#"
fn main() -> i32 {
    let mut v: Vec<i64> = Vec::new();
    v.push(42);
    println!("{}", v.len());
    0
}
"#,
        "1\n",
    );
}

#[test]
fn stage18_197_vec_push_u8() {
    assert_runtime(
        "vec-push-u8",
        r#"
fn main() -> i32 {
    let mut v: Vec<u8> = Vec::new();
    v.push(255);
    println!("{}", v.len());
    0
}
"#,
        "1\n",
    );
}

#[test]
fn stage18_197_vec_push_large_growth() {
    assert_runtime(
        "vec-push-large-growth",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    v.push(4);
    v.push(5);
    v.push(6);
    v.push(7);
    v.push(8);
    v.push(9);
    println!("{} {}", v.len(), v.cap);
    0
}
"#,
        "9 16\n",
    );
}
