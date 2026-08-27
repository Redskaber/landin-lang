//! Stage 18.195 — Vec<T> MVP tests.
//!
//! Verifies:
//! 1. `Vec::new()` creates an empty Vec with len = 0.
//! 2. `Vec::len()` returns the current length.
//! 3. Vec is available via prelude (no import needed).
//! 4. Vec::push compiles (stub — doesn't actually push yet).

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;

#[test]
fn stage18_195_vec_new_empty() {
    assert_runtime(
        "vec-new-empty",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    println!("{}", v.len());
    0
}
"#,
        "0\n",
    );
}

#[test]
fn stage18_195_vec_new_field_access() {
    assert_runtime(
        "vec-new-field-access",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    println!("{} {}", v.len, v.cap);
    0
}
"#,
        "0 0\n",
    );
}

#[test]
fn stage18_195_vec_new_no_import() {
    assert_runtime(
        "vec-new-no-import",
        r#"
fn main() -> i32 {
    let mut v: Vec<i64> = Vec::new();
    println!("{}", v.len());
    0
}
"#,
        "0\n",
    );
}

#[test]
fn stage18_195_vec_multiple() {
    assert_runtime(
        "vec-multiple",
        r#"
fn main() -> i32 {
    let v1: Vec<i32> = Vec::new();
    let v2: Vec<i32> = Vec::new();
    println!("{} {}", v1.len(), v2.len());
    0
}
"#,
        "0 0\n",
    );
}
