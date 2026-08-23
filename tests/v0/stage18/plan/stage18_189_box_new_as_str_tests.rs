//! Stage 18.189 — Box::new + String::as_str tests.
//!
//! Verifies:
//! 1. `Box::new(x)` allocates and stores x, accessible via `*b.0`.
//! 2. `Box::new` works with different types (i32, u8, i64).
//! 3. `String::as_str()` returns a &str with correct length.
//! 4. `String::as_str()` + str methods (len, is_empty) work together.
//! 5. Multiple Box::new calls create independent allocations.

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
    let lin_file = std::env::temp_dir().join(format!(
        "landin_s189_test_{}_{}.lin",
        std::process::id(),
        id
    ));
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

// === Box::new tests ===

#[test]
fn stage18_189_box_new_i32() {
    assert_runtime(
        "box-new-i32",
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

#[test]
fn stage18_189_box_new_u8() {
    assert_runtime(
        "box-new-u8",
        r#"
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let b: Box<u8> = Box::new(255);
    let v: u8 = *b.0;
    println!("{}", v);
    __landin_dealloc(b.0 as *mut u8);
    0
}
"#,
        "255\n",
    );
}

/// Stage 18.190 (TD-BOX-NEW-TYPE-COERCE fix): Box<i64> now works correctly
/// for values that fit in i32 (the pointer bitcast fix in emit_store handles
/// the *mut u8 → *mut i64 cast). Large i64 values (> i32 max) are a separate
/// pre-existing issue (TD-INT-UINT-VAR: literals default to i32).
#[test]
fn stage18_189_box_new_i64() {
    assert_runtime(
        "box-new-i64",
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

#[test]
fn stage18_189_box_new_multiple() {
    assert_runtime(
        "box-new-multiple",
        r#"
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let b1: Box<i32> = Box::new(10);
    let b2: Box<i32> = Box::new(20);
    println!("{} {}", *b1.0, *b2.0);
    __landin_dealloc(b1.0 as *mut u8);
    __landin_dealloc(b2.0 as *mut u8);
    0
}
"#,
        "10 20\n",
    );
}

// === String::as_str tests ===

#[test]
fn stage18_189_string_as_str_len() {
    assert_runtime(
        "string-as-str-len",
        r#"
fn main() -> i32 {
    let s: String = String::from_str("hello");
    let sref: &str = s.as_str();
    println!("{}", sref.len());
    0
}
"#,
        "5\n",
    );
}

#[test]
fn stage18_189_string_as_str_is_empty() {
    assert_runtime(
        "string-as-str-is-empty",
        r#"
fn main() -> i32 {
    let s: String = String::from_str("Hello, World!");
    let sref: &str = s.as_str();
    println!("{} {}", sref.len(), sref.is_empty());
    0
}
"#,
        "13 false\n",
    );
}

#[test]
fn stage18_189_string_as_str_empty() {
    assert_runtime(
        "string-as-str-empty",
        r#"
fn main() -> i32 {
    let s: String = String::new();
    let sref: &str = s.as_str();
    println!("{} {}", sref.len(), sref.is_empty());
    0
}
"#,
        "0 true\n",
    );
}

#[test]
fn stage18_189_string_as_str_byte_index() {
    assert_runtime(
        "string-as-str-byte-index",
        r#"
fn main() -> i32 {
    let s: String = String::from_str("hello");
    let sref: &str = s.as_str();
    println!("{}", sref[0]);
    println!("{}", sref[4]);
    0
}
"#,
        "104\n111\n", // 'h' 'o'
    );
}

// === Box::new + String combined ===

#[test]
fn stage18_189_box_and_string_combined() {
    assert_runtime(
        "box-and-string-combined",
        r#"
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let b: Box<i32> = Box::new(42);
    let s: String = String::from_str("hello");
    let sref: &str = s.as_str();
    println!("{} {}", *b.0, sref.len());
    __landin_dealloc(b.0 as *mut u8);
    0
}
"#,
        "42 5\n",
    );
}
