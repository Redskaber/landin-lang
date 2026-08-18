//! Stage 18.198 — String::push_str tests.
//!
//! Verifies:
//! 1. `s.push_str(" world")` appends to an existing String.
//! 2. `String::new().push_str(...)` works (growth from 0).
//! 3. Multiple push_str calls accumulate correctly.
//! 4. push_str growth (cap 0→4→8→16→...) works.

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
        std::env::temp_dir().join(format!("landin_strpush_{}_{}.lin", std::process::id(), id));
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
fn stage18_198_push_str_append() {
    assert_runtime(
        "push-str-append",
        r#"
fn main() -> i32 {
    let mut s: String = String::from_str("hello");
    s.push_str(" world");
    println!("{}", s.len());
    0
}
"#,
        "11\n",
    );
}

#[test]
fn stage18_198_push_str_from_empty() {
    assert_runtime(
        "push-str-from-empty",
        r#"
fn main() -> i32 {
    let mut s: String = String::new();
    s.push_str("hello");
    println!("{}", s.len());
    0
}
"#,
        "5\n",
    );
}

#[test]
fn stage18_198_push_str_multiple() {
    assert_runtime(
        "push-str-multiple",
        r#"
fn main() -> i32 {
    let mut s: String = String::new();
    s.push_str("Hello");
    s.push_str(", ");
    s.push_str("World!");
    println!("{}", s.len());
    0
}
"#,
        "13\n",
    );
}

#[test]
fn stage18_198_push_str_growth() {
    assert_runtime(
        "push-str-growth",
        r#"
fn main() -> i32 {
    let mut s: String = String::new();
    s.push_str("Hello");
    s.push_str(", ");
    s.push_str("World!");
    println!("{} {}", s.len(), s.cap);
    0
}
"#,
        "13 16\n",
    );
}

#[test]
fn stage18_198_push_str_empty_src() {
    assert_runtime(
        "push-str-empty-src",
        r#"
fn main() -> i32 {
    let mut s: String = String::from_str("hello");
    s.push_str("");
    println!("{}", s.len());
    0
}
"#,
        "5\n",
    );
}

#[test]
fn stage18_198_push_str_long() {
    assert_runtime(
        "push-str-long",
        r#"
fn main() -> i32 {
    let mut s: String = String::new();
    s.push_str("The quick brown fox jumps over the lazy dog");
    println!("{}", s.len());
    0
}
"#,
        "43\n",
    );
}
