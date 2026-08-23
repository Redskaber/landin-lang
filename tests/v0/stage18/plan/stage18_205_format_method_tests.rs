//! Stage 18.205 — TD-FUNCTION-REDEFINE-PARAMS fix tests.
//!
//! Verifies that `format!` result method calls (e.g., `s.len()`) work
//! correctly after the int→ptr store fix. Previously, `ConstVal::Int(0)`
//! used in pointer-typed contexts (e.g., `null` for `*mut u8`) was stored
//! as a 4-byte `i32 0` instead of an 8-byte `ptr null`, leaving upper
//! bytes as stack garbage. When loaded as an 8-byte pointer and passed
//! to C functions, the garbage upper bits caused ABI mismatches → segfault.
//!
//! Per §17.5.2 (test organization): one file per stage.
//! Per §9.4 (设计-开发-测试锚定): tests verify the design intent of
//! proper 8-byte pointer stores.
//! Per §9.4.3 (1:3+ 正负比例): positive tests + regression test for
//! the previously-segfaulting case.

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
        std::env::temp_dir().join(format!("landin_s205_{}_{}.lin", std::process::id(), id));
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

/// Regression: `format!("x={}", 42).len()` — previously segfaulted.
/// This was the canonical TD-FUNCTION-REDEFINE-PARAMS bug: the `null`
/// pointer passed as `arg_types` to `__landin_format_variadic` had
/// garbage in the upper 4 bytes (due to 4-byte `movl` store instead of
/// 8-byte `movq`), causing the C function to take the wrong branch
/// and dereference a garbage pointer.
#[test]
fn stage18_205_format_len_method_call() {
    assert_runtime(
        "format-len-method",
        r#"
fn main() -> i32 {
    let s = format!("x={}", 42);
    println!("{}", s.len());
    0
}
"#,
        "4\n",
    );
}

/// Regression: `format!` result + method call with intermediate binding.
#[test]
fn stage18_205_format_len_via_binding() {
    assert_runtime(
        "format-len-binding",
        r#"
fn main() -> i32 {
    let s = format!("x={}", 42);
    let n = s.len();
    println!("{}", n);
    0
}
"#,
        "4\n",
    );
}

/// Regression: field access followed by method call (both work).
#[test]
fn stage18_205_format_field_then_method() {
    assert_runtime(
        "format-field-then-method",
        r#"
fn main() -> i32 {
    let s = format!("x={}", 42);
    println!("{}", s.len);
    println!("{}", s.len());
    0
}
"#,
        "4\n4\n",
    );
}

/// Regression: `String::from_str` + method call (was already working,
/// but verify no regression from the store fix).
#[test]
fn stage18_205_from_str_method_call() {
    assert_runtime(
        "from-str-method",
        r#"
fn main() -> i32 {
    let s = String::from_str("hello");
    println!("{}", s.len());
    0
}
"#,
        "5\n",
    );
}

/// Regression: `String::new()` + method call (was already working).
#[test]
fn stage18_205_string_new_method_call() {
    assert_runtime(
        "string-new-method",
        r#"
fn main() -> i32 {
    let s = String::new();
    println!("{}", s.len());
    0
}
"#,
        "0\n",
    );
}

/// Regression: `format!` with multiple args + method call.
#[test]
fn stage18_205_format_multi_args_len() {
    assert_runtime(
        "format-multi-args",
        r#"
fn main() -> i32 {
    let s = format!("{}+{}={}", 1, 2, 3);
    println!("{}", s.len());
    0
}
"#,
        "5\n",
    );
}

/// Regression: Box::new + Deref (was already working, verify no regression).
#[test]
fn stage18_205_box_new_deref() {
    assert_runtime(
        "box-new-deref",
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

/// Regression: format! + cap field (verify cap is correct).
#[test]
fn stage18_205_format_cap_field() {
    assert_runtime(
        "format-cap-field",
        r#"
fn main() -> i32 {
    let s = format!("x={}", 42);
    println!("{}", s.cap);
    0
}
"#,
        "5\n",
    );
}
