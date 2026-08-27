//! Stage 18.236 — Pointer Arithmetic language feature tests.
//!
//! Verifies that `ptr + int` and `ptr - int` are accepted by typeck
//! and lowered to GetElementPtr in MIR. This is the prerequisite for
//! TD-INTRINSIC-OVERUSE migration (stdlib impl blocks using pointer
//! arithmetic instead of hardcoded MIR lower intrinsics).
//!
//! Per §9.4.3 (1:3+ 正负比例): positive (valid arithmetic) + negative (invalid).

#![cfg(all(test, feature = "llvm-backend"))]

use std::path::Path;
#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::run_program;
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
        std::env::temp_dir().join(format!("landin_ptrarith_{}_{}.lin", std::process::id(), id));
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
// POSITIVE TESTS — valid pointer arithmetic compiles.
// =========================================================================

/// Positive 1: `p + 1` where p is *mut u8 — should compile.
#[test]
fn stage18_236_ptr_add_int() {
    let code = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(16);
    let q: *mut u8 = p + 1;
    0
}
"#;
    let exit = compile_only(code);
    assert_eq!(exit, 0, "ptr + int should compile");
}

/// Positive 2: `p + 0` is a no-op offset.
#[test]
fn stage18_236_ptr_add_zero() {
    let code = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(16);
    let q: *mut u8 = p + 0;
    0
}
"#;
    let exit = compile_only(code);
    assert_eq!(exit, 0, "ptr + 0 should compile");
}

/// Positive 3: `p - 1` backward offset.
#[test]
fn stage18_236_ptr_sub_int() {
    let code = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(16);
    let q: *mut u8 = p + 2;
    let r: *mut u8 = q - 1;
    0
}
"#;
    let exit = compile_only(code);
    assert_eq!(exit, 0, "ptr - int should compile");
}

/// Positive 4: `p + n` where n is a variable.
#[test]
fn stage18_236_ptr_add_var() {
    let code = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(16);
    let n: i64 = 3;
    let q: *mut u8 = p + n;
    0
}
"#;
    let exit = compile_only(code);
    assert_eq!(exit, 0, "ptr + var should compile");
}

/// Positive 5: `int + ptr` (reversed operand order).
#[test]
fn stage18_236_int_add_ptr() {
    let code = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(16);
    let q: *mut u8 = 1 + p;
    0
}
"#;
    let exit = compile_only(code);
    assert_eq!(exit, 0, "int + ptr should compile");
}

// =========================================================================
// NEGATIVE TESTS — invalid pointer arithmetic fails.
// =========================================================================

/// Negative 1: `p + q` (ptr + ptr) should fail.
#[test]
fn stage18_236_ptr_add_ptr_fails() {
    let code = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(16);
    let q: *mut u8 = __landin_alloc(16);
    let r: *mut u8 = p + q;
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(exit, 0, "ptr + ptr should fail");
}

/// Negative 2: `p * 2` (ptr * int) should fail.
#[test]
fn stage18_236_ptr_mul_int_fails() {
    let code = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(16);
    let q: *mut u8 = p * 2;
    0
}
"#;
    let exit = compile_only(code);
    assert_ne!(exit, 0, "ptr * int should fail");
}

// =========================================================================
// RUNTIME TESTS — Store/Load through pointer arithmetic (Stage 18.237 fix)
// =========================================================================

/// Stage 18.237: Store and load through `*(p + 0)`.
#[test]
fn stage18_237_store_load_through_offset_zero() {
    let (stdout, exit) = run_program(
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let p: *mut i32 = __landin_alloc(16) as *mut i32;
    *(p + 0) = 42;
    println!("{}", *(p + 0));
    0
}
"#,
    );
    assert_eq!(stdout, "42\n");
    assert_eq!(exit, 0);
}

/// Stage 18.237: Store and load through multiple offsets.
#[test]
fn stage18_237_store_load_multiple_offsets() {
    let (stdout, exit) = run_program(
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let p: *mut i32 = __landin_alloc(16) as *mut i32;
    *(p + 0) = 10;
    *(p + 1) = 20;
    *(p + 2) = 30;
    println!("{}", *(p + 0));
    println!("{}", *(p + 1));
    println!("{}", *(p + 2));
    0
}
"#,
    );
    assert_eq!(stdout, "10\n20\n30\n");
    assert_eq!(exit, 0);
}

/// Stage 18.237: Store through variable offset.
#[test]
fn stage18_237_store_through_variable_offset() {
    let (stdout, exit) = run_program(
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let p: *mut i32 = __landin_alloc(16) as *mut i32;
    let i: i64 = 1;
    *(p + 0) = 99;
    *(p + i) = 100;
    println!("{}", *(p + 0));
    println!("{}", *(p + 1));
    0
}
"#,
    );
    assert_eq!(stdout, "99\n100\n");
    assert_eq!(exit, 0);
}
