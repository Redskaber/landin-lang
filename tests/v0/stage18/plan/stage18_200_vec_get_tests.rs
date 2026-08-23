//! Stage 18.200 — Vec::get tests.
//!
//! Verifies:
//! 1. `v.get(0)` returns the first element.
//! 2. `v.get(N)` returns elements at various indices.
//! 3. `v.get(N)` panics on OOB.
//! 4. Vec::get after multiple pushes works.

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
        std::env::temp_dir().join(format!("landin_vecget_{}_{}.lin", std::process::id(), id));
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
fn stage18_200_vec_get_first() {
    assert_runtime(
        "vec-get-first",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(10);
    v.push(20);
    v.push(30);
    let x = v.get(0);
    println!("{}", x);
    0
}
"#,
        "10\n",
    );
}

#[test]
fn stage18_200_vec_get_all() {
    assert_runtime(
        "vec-get-all",
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

#[test]
fn stage18_200_vec_get_after_growth() {
    assert_runtime(
        "vec-get-after-growth",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    v.push(4);
    v.push(5);
    println!("{}", v.get(4));
    0
}
"#,
        "5\n",
    );
}

#[test]
fn stage18_200_vec_get_oob_panics() {
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
    assert_ne!(exit, 0, "expected OOB panic (exit != 0), got exit {}", exit);
}
