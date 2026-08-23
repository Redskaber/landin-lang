//! Stage 18.194 — Realloc infrastructure tests.
//!
//! Verifies that:
//! 1. `__landin_realloc(ptr, old, new)` works — preserves data after resize.
//! 2. Realloc to larger size preserves existing data.
//! 3. Realloc to smaller size preserves existing data (up to new size).
//! 4. Multiple reallocs in sequence work.
//! 5. Realloc'd memory can be freed with __landin_dealloc.

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
        "landin_realloc_test_{}_{}.lin",
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
    assert_eq!(stdout, expected_stdout, "Test '{}': stdout mismatch", name);
    assert_eq!(
        exit, 0,
        "Test '{}': exit code mismatch (expected 0, got {})",
        name, exit
    );
}

#[test]
fn stage18_194_realloc_preserves_data_grow() {
    assert_runtime(
        "realloc-preserves-data-grow",
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_realloc(ptr: *mut u8, old: i64, new: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(4);
    *p = 42;
    let p2: *mut u8 = __landin_realloc(p, 4, 8);
    let v: u8 = *p2;
    println!("{}", v);
    __landin_dealloc(p2);
    0
}
"#,
        "42\n",
    );
}

#[test]
fn stage18_194_realloc_preserves_data_shrink() {
    assert_runtime(
        "realloc-preserves-data-shrink",
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_realloc(ptr: *mut u8, old: i64, new: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(8);
    *p = 99;
    let p2: *mut u8 = __landin_realloc(p, 8, 4);
    let v: u8 = *p2;
    println!("{}", v);
    __landin_dealloc(p2);
    0
}
"#,
        "99\n",
    );
}

#[test]
fn stage18_194_realloc_chain() {
    assert_runtime(
        "realloc-chain",
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_realloc(ptr: *mut u8, old: i64, new: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(2);
    *p = 10;
    let p2: *mut u8 = __landin_realloc(p, 2, 4);
    *p2 = 20;
    let p3: *mut u8 = __landin_realloc(p2, 4, 8);
    let v1: u8 = *p3;
    *p3 = 30;
    let v2: u8 = *p3;
    println!("{} {}", v1, v2);
    __landin_dealloc(p3);
    0
}
"#,
        "20 30\n",
    );
}

#[test]
fn stage18_194_realloc_null_oom() {
    // Realloc with NULL ptr should behave like alloc.
    // Realloc to 0 should be safe (returns valid or NULL).
    assert_runtime(
        "realloc-null-oom",
        r#"
extern "C" { fn __landin_realloc(ptr: *mut u8, old: i64, new: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let null: *mut u8 = 0 as *mut u8;
    let p: *mut u8 = __landin_realloc(null, 0, 4);
    *p = 7;
    println!("{}", *p);
    __landin_dealloc(p);
    0
}
"#,
        "7\n",
    );
}
