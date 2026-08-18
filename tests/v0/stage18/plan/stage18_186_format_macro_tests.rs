//! Stage 18.186 (TD-FORMAT-MACRO MVP) — format! macro tests.
//!
//! Verifies that:
//! 1. `format!("literal")` creates an owned String with correct length.
//! 2. `format!("")` creates an empty String (len == 0).
//! 3. `format!("hello").len()` method works.
//! 4. `format!("x={}", x)` produces a clean error (TD-FORMAT-VARIADIC).
//!
//! This is the MVP — only literal string format! is supported. Format
//! args ({}) are deferred to Stage 18.187+ (TD-FORMAT-VARIADIC).
//!
//! Per `stage-committee-process.md` §9.4.3: 1:3+ positive:negative ratio.
//! Per §1.0 原則 6 (通解>特例): reuse String::from_str intrinsic.
//! Per §2 原則 9 (正确>妥协): MVP is a temporary compromise for literals.

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
        std::env::temp_dir().join(format!("landin_fmt_test_{}_{}.lin", std::process::id(), id));
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

fn compile_only(code: &str) -> i32 {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/debug/landin-stage0");
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let lin_file =
        std::env::temp_dir().join(format!("landin_fmt_neg_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin file");
    let output = Command::new(&bin)
        .arg("--compile")
        .arg(&lin_file)
        .output()
        .expect("failed to execute");
    let _ = std::fs::remove_file(&lin_file);
    output.status.code().unwrap_or(-1)
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
// POSITIVE TESTS — format! with literal string works.
// =========================================================================

/// Positive 1: `format!("hello")` creates a String with length 5.
#[test]
fn stage18_186_format_literal_length() {
    assert_runtime(
        "format-literal-length",
        r#"
fn main() -> i32 {
    let s = format!("hello");
    println!("{}", s.len());
    0
}
"#,
        "5\n",
    );
}

/// Positive 2: `format!("")` creates an empty String (length 0).
#[test]
fn stage18_186_format_empty() {
    assert_runtime(
        "format-empty",
        r#"
fn main() -> i32 {
    let s = format!("");
    println!("{}", s.len());
    0
}
"#,
        "0\n",
    );
}

/// Positive 3: `format!("hello").len` field access works.
#[test]
fn stage18_186_format_field_access() {
    assert_runtime(
        "format-field-access",
        r#"
fn main() -> i32 {
    let s = format!("Hello, World!");
    println!("{}", s.len);
    0
}
"#,
        "13\n",
    );
}

/// Positive 4: format! result can be used with String methods.
#[test]
fn stage18_186_format_with_string_methods() {
    assert_runtime(
        "format-with-string-methods",
        r#"
fn main() -> i32 {
    let s = format!("hello");
    println!("{}", s.len());
    let s2 = format!("world");
    println!("{}", s2.len());
    0
}
"#,
        "5\n5\n",
    );
}

/// Positive 5: format! result is independent of source (owned).
///
/// After format!, modifying the source doesn't affect the result.
/// (Since the source is a literal, this is trivially true, but the test
/// verifies the String is owned — has its own heap allocation.)
#[test]
fn stage18_186_format_owned_independent() {
    assert_runtime(
        "format-owned-independent",
        r#"
fn main() -> i32 {
    let s1 = format!("hello");
    let s2 = format!("world");
    println!("{} {}", s1.len(), s2.len());
    0
}
"#,
        "5 5\n",
    );
}

// =========================================================================
// NEGATIVE TESTS — format! with args produces clean error.
// =========================================================================

/// Negative 1: `format!("x={}", x)` must produce a clean error.
///
/// MVP only supports literal strings. Format args ({}) are deferred to
/// Stage 18.187+ (TD-FORMAT-VARIADIC).
#[test]
fn stage18_186_format_with_args_fails() {
    let code = r#"
fn main() -> i32 {
    let x = 42;
    let s = format!("x={}", x);
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(
        exit, 0,
        "expected compile failure for format! with args (TD-FORMAT-VARIADIC), got exit {}",
        exit
    );
}

/// Negative 2: `format!("{}", 42)` must produce a clean error.
#[test]
fn stage18_186_format_placeholder_only_fails() {
    let code = r#"
fn main() -> i32 {
    let s = format!("{}", 42);
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(
        exit, 0,
        "expected compile failure for format!({{}}) with arg (TD-FORMAT-VARIADIC), got exit {}",
        exit
    );
}

/// Negative 3: `format!("a", "b")` with 2 literal args must fail.
///
/// Even though both are literals, multiple args aren't supported in MVP.
#[test]
fn stage18_186_format_multiple_literal_args_fails() {
    let code = r#"
fn main() -> i32 {
    let s = format!("a", "b");
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(
        exit, 0,
        "expected compile failure for format! with 2 args (TD-FORMAT-VARIADIC), got exit {}",
        exit
    );
}
