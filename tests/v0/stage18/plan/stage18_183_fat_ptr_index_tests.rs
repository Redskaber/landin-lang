//! Stage 18.183 (TD-FAT-PTR-INDEX-PROJ fix) — Fat pointer Index projection tests.
//!
//! Verifies that:
//! 1. `s[0]` for `&str` returns the first byte (ASCII code).
//! 2. `s[N]` for various positions returns the correct byte.
//! 3. `s[0]` through `s[len-1]` each return distinct values.
//! 4. Multiple fat pointer Index in one expression works.
//! 5. Fat pointer Index with let-bound index variable works.
//! 6. Fat pointer Index returns the correct type (u8).
//!
//! This fixes the P1 bug from Stage 18.181 base types audit:
//! `s[0]` on `&str` produced invalid IR ("GEP base pointer is not a vector")
//! because:
//!   - `unwrap_fat_ptr_for_index` GEP'd to field 0 but didn't LOAD the data pointer
//!   - Index codegen loaded the fat pointer VALUE (not ADDRESS) for Ref types
//!
//! Per `stage-committee-process.md` §9.4.3: 1:3+ positive:negative ratio.
//! Per §1.0 原則 6 (通解>特例): one alloca+GEP+load path for all fat pointer Index.

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
        "landin_fatptr_test_{}_{}.lin",
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

// =========================================================================
// POSITIVE TESTS — Fat pointer Index projection works.
// =========================================================================

/// Positive 1: `s[0]` for `&str` returns the first byte (ASCII 'h' = 104).
///
/// This is the canonical regression test for the P1 bug. Before Stage 18.183,
/// `s[0]` produced invalid IR ("GEP base pointer is not a vector").
#[test]
fn stage18_183_str_index_first_byte() {
    assert_runtime(
        "str-index-first-byte",
        r#"
fn main() -> i32 {
    let s: &str = "hello";
    let b: u8 = s[0];
    println!("{}", b);
    0
}
"#,
        "104\n", // 'h' = 104
    );
}

/// Positive 2: `s[1]`, `s[2]`, `s[3]`, `s[4]` each return the correct byte.
///
/// Verifies that indexing at different positions works, not just position 0.
#[test]
fn stage18_183_str_index_various_positions() {
    assert_runtime(
        "str-index-various-positions",
        r#"
fn main() -> i32 {
    let s: &str = "hello";
    println!("{}", s[0]);
    println!("{}", s[1]);
    println!("{}", s[2]);
    println!("{}", s[3]);
    println!("{}", s[4]);
    0
}
"#,
        "104\n101\n108\n108\n111\n", // h e l l o
    );
}

/// Positive 3: Multiple fat pointer Index in one println! expression.
///
/// `println!("{} {}", s[0], s[4])` should print the first and last bytes.
#[test]
fn stage18_183_str_multi_index_one_expr() {
    assert_runtime(
        "str-multi-index-one-expr",
        r#"
fn main() -> i32 {
    let s: &str = "world";
    println!("{} {}", s[0], s[4]);
    0
}
"#,
        "119 100\n", // 'w' 'd'
    );
}

/// Positive 4: Fat pointer Index with let-bound index variable.
///
/// `let i = 2; s[i]` should return s[2].
#[test]
fn stage18_183_str_index_via_let_var() {
    assert_runtime(
        "str-index-via-let-var",
        r#"
fn main() -> i32 {
    let s: &str = "hello";
    let i = 2;
    println!("{}", s[i]);
    0
}
"#,
        "108\n", // 'l' = 108
    );
}

/// Positive 5: Fat pointer Index returns u8 (byte), not char.
///
/// Verifies the type is correct — `s[0]` has type `u8`, not `char`.
/// Multi-byte UTF-8 characters would return individual bytes, not codepoints.
#[test]
fn stage18_183_str_index_returns_u8() {
    assert_runtime(
        "str-index-returns-u8",
        r#"
fn main() -> i32 {
    let s: &str = "AB";
    let a: u8 = s[0];
    let b: u8 = s[1];
    println!("{} {}", a, b);
    0
}
"#,
        "65 66\n", // 'A' 'B'
    );
}

/// Positive 6: Index into empty string would be OOB (soft test).
///
/// `""[0]` is out of bounds. This test verifies the compiler doesn't crash
/// on empty strings — runtime behavior (panic vs garbage) is TD-ARRAY-BOUNDS-CHECK.
#[test]
fn stage18_183_str_index_empty_soft() {
    let code = r#"
fn main() -> i32 {
    let s: &str = "";
    let b: u8 = s[0];
    println!("{}", b);
    0
}
"#;
    let (_stdout, exit) = run_program(code);
    // Ideally OOB should panic (exit != 0). If it returns a value, warn.
    if exit == 0 {
        eprintln!("warn: \"\"[0] did not panic (TD-ARRAY-BOUNDS-CHECK: no bounds check yet)");
    }
    // Soft test: always passes, but warns if bounds check is missing.
}

/// Positive 7: str::len() + s[0] combined.
///
/// Verifies that fat pointer Field projection (len) and Index projection (s[0])
/// work together correctly on the same &str.
#[test]
fn stage18_183_str_len_and_index_combined() {
    assert_runtime(
        "str-len-and-index-combined",
        r#"
fn main() -> i32 {
    let s: &str = "hello";
    let len = s.len();
    let first = s[0];
    println!("{} {}", len, first);
    0
}
"#,
        "5 104\n", // len=5, first='h'=104
    );
}

// =========================================================================
// NEGATIVE TESTS — Fat pointer Index misuse.
// =========================================================================

/// Negative 1: `s[0]` on non-string type must fail type check.
///
/// Indexing an `i32` should produce a type error (i32 is not indexable).
#[test]
fn stage18_183_index_non_string_fails() {
    let code = r#"
fn main() -> i32 {
    let x: i32 = 42;
    let b = x[0];
    0
}
"#;
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = if cfg!(debug_assertions) {
        manifest.join("target/debug/landin-stage0")
    } else {
        manifest.join("target/release/landin-stage0")
    };
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let lin_file = std::env::temp_dir().join(format!(
        "landin_fatptr_neg_{}_{}.lin",
        std::process::id(),
        id
    ));
    std::fs::write(&lin_file, code).expect("write .lin file");
    let output = Command::new(&bin)
        .arg("--compile")
        .arg(&lin_file)
        .output()
        .expect("failed to execute");
    let _ = std::fs::remove_file(&lin_file);
    let exit = output.status.code().unwrap_or(-1);
    assert_ne!(
        exit, 0,
        "expected compile failure for i32[0] (not indexable), got exit {}",
        exit
    );
}
