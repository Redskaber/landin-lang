//! Stage 18.234 — TD-METHOD-RESOLVE-STRICT regression tests.
//!
//! Verifies that method calls on Infer-typed receivers are correctly
//! resolved after typeck defaulting. Specifically:
//! - `s.nonexistent_method()` where `s` comes from `String::new()` (Infer
//!   at MIR lower time) should report "no method found".
//! - Valid intrinsic methods (push_str, len, etc.) on Infer receivers
//!   should NOT report false positives.
//!
//! Per §9.4.3 (1:3+ 正负比例): positive (valid methods) + negative (unknown methods).

#![cfg(all(test, feature = "llvm-backend"))]

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn compile_only(code: &str) -> i32 {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = if cfg!(debug_assertions) {
        manifest.join("target/debug/landin-stage0")
    } else {
        manifest.join("target/release/landin-stage0")
    };
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let lin_file =
        std::env::temp_dir().join(format!("landin_method_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin file");
    let output = Command::new(&bin)
        .arg("--compile")
        .arg(&lin_file)
        .output()
        .expect("failed to execute");
    let _ = std::fs::remove_file(&lin_file);
    output.status.code().unwrap_or(-1)
}

// =========================================================================
// POSITIVE TESTS — valid methods on Infer receivers work.
// =========================================================================

/// Positive 1: `String::new().len()` — len is a valid intrinsic method.
#[test]
fn stage18_234_infer_recv_valid_len() {
    let code = r#"
fn main() -> i32 {
    let s: String = String::new();
    let n = s.len();
    0
}
"#;
    let exit = compile_only(code);
    assert_eq!(exit, 0, "valid method len on String should compile");
}

/// Positive 2: `String::new().push_str(...)` — push_str is a valid intrinsic.
#[test]
fn stage18_234_infer_recv_valid_push_str() {
    let code = r#"
fn main() -> i32 {
    let mut s: String = String::new();
    s.push_str("hello");
    0
}
"#;
    let exit = compile_only(code);
    assert_eq!(exit, 0, "valid method push_str on String should compile");
}

/// Positive 3: `Vec::new().push(...)` — push is a valid intrinsic.
#[test]
fn stage18_234_infer_recv_valid_push() {
    let code = r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(42);
    0
}
"#;
    let exit = compile_only(code);
    assert_eq!(exit, 0, "valid method push on Vec should compile");
}

// =========================================================================
// NEGATIVE TESTS — unknown methods on Infer receivers are reported.
// =========================================================================

/// Negative 1: `String::new().nonexistent_method()` — should fail.
#[test]
fn stage18_234_infer_recv_unknown_method() {
    let code = r#"
fn main() -> i32 {
    let s = String::new();
    s.nonexistent_method();
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(exit, 0, "unknown method on Infer receiver should fail");
}

/// Negative 2: `String::new().foobar()` — should fail.
#[test]
fn stage18_234_infer_recv_unknown_method_foobar() {
    let code = r#"
fn main() -> i32 {
    let s = String::new();
    s.foobar(42);
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(
        exit, 0,
        "unknown method foobar on Infer receiver should fail"
    );
}

/// Negative 3: `String::new().xyz()` — should fail.
#[test]
fn stage18_234_infer_recv_unknown_method_xyz() {
    let code = r#"
fn main() -> i32 {
    let s = String::new();
    s.xyz();
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(exit, 0, "unknown method xyz on Infer receiver should fail");
}

// =========================================================================
// REGRESSION TESTS — explicit-type receivers still work.
// =========================================================================

/// Regression: `let s: String = ...; s.nonexistent()` should still fail.
#[test]
fn stage18_234_explicit_recv_unknown_method() {
    let code = r#"
fn main() -> i32 {
    let s: String = String::new();
    s.nonexistent_method();
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(
        exit, 0,
        "unknown method on explicit-type receiver should fail"
    );
}
