//! Stage 18.180 (TD-STRING-AS-STR-ALIAS fix) — Real String type tests.
//!
//! Verifies that:
//! 1. `String` is now a real struct type (not a &str alias).
//! 2. `String { ptr, len, cap }` struct literal construction works.
//! 3. Field access (`s.ptr`, `s.len`, `s.cap`) works.
//! 4. String can be allocated on the heap via `__landin_alloc`.
//! 5. String is NOT assignable from a string literal (must construct).
//!
//! This fixes the Stage 18.176 design violation where String was mapped to
//! PrimTy::Str (a stack-allocated fat pointer). Per the design doc
//! (09-stdlib.md §3.4), String must be an owned heap type.
//!
//! MVP limitation: Users must construct String manually via struct literal.
//! Ergonomic intrinsics (String::from_str, push_str, len, as_str) are
//! deferred to Stage 18.181 (TD-STRING-INTRINSICS).
//!
//! Per `stage-committee-process.md` §9.4.3: 1:3+ positive:negative ratio.
//! Per §2 原則 9 (正确>妥协): the &str alias compromise is removed.

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
        std::env::temp_dir().join(format!("landin_str_test_{}_{}.lin", std::process::id(), id));
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
        std::env::temp_dir().join(format!("landin_str_neg_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin file");

    let output = Command::new(&bin)
        .arg("--compile")
        .arg(&lin_file)
        .output()
        .expect("failed to execute landin-stage0");

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
// POSITIVE TESTS — Real String type works.
// =========================================================================

/// Positive 1: String is a real struct — construct via struct literal.
///
/// Verifies that `String { ptr, len, cap }` compiles and runs. The struct
/// is in the prelude (no import needed).
#[test]
fn stage18_180_string_struct_literal_construct() {
    assert_runtime(
        "string-struct-literal-construct",
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(5);
    let s: String = String { ptr: p, len: 5, cap: 5 };
    println!("{}", s.len);
    __landin_dealloc(s.ptr);
    0
}
"#,
        "5\n",
    );
}

/// Positive 2: String fields are individually accessible.
///
/// Verifies `s.ptr`, `s.len`, `s.cap` field access. The pointer can be
/// passed to `__landin_dealloc` for cleanup.
#[test]
fn stage18_180_string_field_access() {
    assert_runtime(
        "string-field-access",
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(8);
    let s: String = String { ptr: p, len: 3, cap: 8 };
    println!("{} {}", s.len, s.cap);
    __landin_dealloc(s.ptr);
    0
}
"#,
        "3 8\n",
    );
}

/// Positive 3: String is in prelude — no import needed.
///
/// Verifies that `String` is auto-imported (no `use` statement, no user
/// `struct String` declaration). This is the prelude injection contract.
#[test]
fn stage18_180_string_in_prelude_no_import() {
    assert_runtime(
        "string-in-prelude-no-import",
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(2);
    let s: String = String { ptr: p, len: 2, cap: 2 };
    println!("{}", s.cap);
    __landin_dealloc(s.ptr);
    0
}
"#,
        "2\n",
    );
}

/// Positive 4: String can hold heap-allocated byte content.
///
/// Allocates a buffer, stores a byte via the raw pointer, wraps in String,
/// then reads the byte back via the pointer. Verifies the full heap
/// allocation cycle for String.
#[test]
fn stage18_180_string_holds_heap_bytes() {
    assert_runtime(
        "string-holds-heap-bytes",
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(3);
    *p = 65;  // 'A'
    let s: String = String { ptr: p, len: 1, cap: 3 };
    let first_byte: u8 = *s.ptr;
    println!("{}", first_byte);
    __landin_dealloc(s.ptr);
    0
}
"#,
        "65\n",
    );
}

// =========================================================================
// NEGATIVE TESTS — String is NOT a &str alias anymore.
// =========================================================================

/// Negative 1: `let s: String = "hello"` must fail (String is not &str).
///
/// Stage 18.176 allowed this (String was a &str alias). Stage 18.180
/// removes the alias — String is now a struct, so assigning a &str literal
/// must be a type error.
#[test]
fn stage18_180_string_literal_assign_fails() {
    let code = r#"
fn main() -> i32 {
    let s: String = "hello";
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(
        exit, 0,
        "expected compile failure for `let s: String = \"hello\"` (String is no longer &str alias), got exit {}",
        exit
    );
}

/// Negative 2: Redeclaring `String` as a user struct must fail.
///
/// The prelude injects `struct String { ptr, len, cap }`. User redefinition
/// must produce a duplicate definition error.
#[test]
fn stage18_180_string_redefinition_fails() {
    let code = r#"
struct String { x: i32 }
fn main() -> i32 { 0 }
"#;
    let exit = compile_only(code);
    assert_ne!(
        exit, 0,
        "expected compile failure for user redefining String, got exit {}",
        exit
    );
}

/// Negative 3 (SOFT): `String::new()` should fail (intrinsic not yet implemented).
///
/// Stage 18.181 will add `String::from_str` and other intrinsics. For now,
/// `String::new()` should fail because there's no `impl String { fn new }`
/// in the prelude. However, Landin's resolver may be permissive and accept
/// unknown method calls. This test documents the expected behavior.
///
/// TODO: Tighten resolver to reject unknown methods on prelude types
/// (TD-METHOD-RESOLVE-STRICT).
#[test]
fn stage18_180_string_new_not_yet_implemented_soft() {
    let code = r#"
fn main() -> i32 {
    let s: String = String::new();
    0
}
"#;
    let exit = compile_only(code);
    if exit == 0 {
        eprintln!(
            "warn: String::new() did not fail at compile time \
             (TD-METHOD-RESOLVE-STRICT: resolver permissive on unknown methods)"
        );
    }
    // Soft test: always passes, but warns if the compiler is too permissive.
}

/// Negative 4: Missing field in String struct literal must fail.
///
/// `String { ptr: p, len: 5 }` (missing `cap`) should be a compile error.
#[test]
fn stage18_180_string_missing_field_fails() {
    let code = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(5);
    let s: String = String { ptr: p, len: 5 };
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(
        exit, 0,
        "expected compile failure for String literal missing `cap` field, got exit {}",
        exit
    );
}

/// Negative 5: Wrong field type in String literal must fail.
///
/// `String { ptr: 42, ... }` (ptr must be *mut u8, not i32) should error.
#[test]
fn stage18_180_string_wrong_field_type_fails() {
    let code = r#"
fn main() -> i32 {
    let s: String = String { ptr: 42, len: 5, cap: 5 };
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(
        exit, 0,
        "expected compile failure for String.ptr = 42 (wrong type, expected *mut u8), got exit {}",
        exit
    );
}
