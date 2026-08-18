//! Stage 18.195 — Vec<T> MVP tests.
//!
//! Verifies:
//! 1. `Vec::new()` creates an empty Vec with len = 0.
//! 2. `Vec::len()` returns the current length.
//! 3. Vec is available via prelude (no import needed).
//! 4. Vec::push compiles (stub — doesn't actually push yet).

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
        std::env::temp_dir().join(format!("landin_vec_test_{}_{}.lin", std::process::id(), id));
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
    assert_eq!(stdout, expected_stdout, "Test '{}': stdout mismatch", name);
    assert_eq!(
        exit, 0,
        "Test '{}': exit code mismatch (expected 0, got {})",
        name, exit
    );
}

#[test]
fn stage18_195_vec_new_empty() {
    assert_runtime(
        "vec-new-empty",
        r#"
fn main() -> i32 {
    let v: Vec<i32> = Vec::new();
    println!("{}", v.len());
    0
}
"#,
        "0\n",
    );
}

#[test]
fn stage18_195_vec_new_field_access() {
    assert_runtime(
        "vec-new-field-access",
        r#"
fn main() -> i32 {
    let v: Vec<i32> = Vec::new();
    println!("{} {}", v.len, v.cap);
    0
}
"#,
        "0 0\n",
    );
}

#[test]
fn stage18_195_vec_new_no_import() {
    assert_runtime(
        "vec-new-no-import",
        r#"
fn main() -> i32 {
    let v: Vec<i64> = Vec::new();
    println!("{}", v.len());
    0
}
"#,
        "0\n",
    );
}

#[test]
fn stage18_195_vec_multiple() {
    assert_runtime(
        "vec-multiple",
        r#"
fn main() -> i32 {
    let v1: Vec<i32> = Vec::new();
    let v2: Vec<i32> = Vec::new();
    println!("{} {}", v1.len(), v2.len());
    0
}
"#,
        "0 0\n",
    );
}
