//! Stage 18.212 — TD-TUPLE-CTOR-TYPECK fix tests.
//!
//! Verifies that `Box<T>::new(x)` correctly carries the element type T
//! in the Box's substs, enabling typeck to accept `Box<Point>` and
//! `Box<i64>` (previously hardcoded to `Box<u8>`).
//!
//! Per Stage 18.207 task review: TD-TUPLE-CTOR-TYPECK was identified as
//! a real typeck issue — `Box<T>(*mut T)` field type wasn't substituted
//! when Box<Point> was used.
//!
//! Stage 18.212 fix:
//! 1. `build_adt_layout` now uses `find_generics` + `lower_hir_ty_to_mir_ty_with_hir_and_generics`
//!    so `struct Box<T>(*mut T)` field type resolves to `Param(0)` instead of `Error`.
//! 2. `lower_box_new_intrinsic` now constructs `Box<T>` with `substs = [val_ty]`
//!    instead of empty substs, and `field_ty = *mut T` instead of `*mut u8`.
//! 3. `alloc_dest` local type is now `*mut T` (matching the value type) so
//!    typeck's `*alloc_dest = x` assignment type-checks correctly.

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
        std::env::temp_dir().join(format!("landin_s212_{}_{}.lin", std::process::id(), id));
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

/// Regression: Box<i64>::new(42i64) — was failing with "expected u8, found i64".
#[test]
fn stage18_212_box_i64_new() {
    assert_runtime(
        "box-i64-new",
        r#"
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let b: Box<i64> = Box::new(42i64);
    let v: i64 = *b.0;
    println!("{}", v);
    __landin_dealloc(b.0 as *mut u8);
    0
}
"#,
        "42\n",
    );
}

/// Regression: Box<i32>::new(42) — canonical case, must still work.
#[test]
fn stage18_212_box_i32_new() {
    assert_runtime(
        "box-i32-new",
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

/// Regression: Box<Point>::new(p) — struct element type.
#[test]
fn stage18_212_box_struct_new() {
    assert_runtime(
        "box-struct-new",
        r#"
struct Point { x: i32, y: i32 }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p = Point { x: 10, y: 20 };
    let b: Box<Point> = Box::new(p);
    let q: Point = *b.0;
    println!("{}", q.x);
    __landin_dealloc(b.0 as *mut u8);
    0
}
"#,
        "10\n",
    );
}

/// Regression: Box<i32> with multiple Box::new calls (independent allocations).
#[test]
fn stage18_212_box_multiple() {
    assert_runtime(
        "box-multiple",
        r#"
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let a: Box<i32> = Box::new(10);
    let b: Box<i32> = Box::new(20);
    println!("{}", *a.0);
    println!("{}", *b.0);
    __landin_dealloc(a.0 as *mut u8);
    __landin_dealloc(b.0 as *mut u8);
    0
}
"#,
        "10\n20\n",
    );
}
