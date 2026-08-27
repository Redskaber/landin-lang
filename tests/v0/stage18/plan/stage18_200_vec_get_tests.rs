//! Stage 18.200 — Vec::get tests.
//!
//! Verifies:
//! 1. `v.get(0)` returns the first element.
//! 2. `v.get(N)` returns elements at various indices.
//! 3. `v.get(N)` panics on OOB.
//! 4. Vec::get after multiple pushes works.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::{assert_runtime, run_program};

#[test]
fn stage18_200_vec_get_first() {
    assert_runtime(
        "vec-get-first",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(10);
    v.push(20);
    v.push(30);
    let x = v.get(0);
    println!("{}", x);
    0
}
"#,
        "10\n",
    );
}

#[test]
fn stage18_200_vec_get_all() {
    assert_runtime(
        "vec-get-all",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(10);
    v.push(20);
    v.push(30);
    println!("{}", v.get(0));
    println!("{}", v.get(1));
    println!("{}", v.get(2));
    0
}
"#,
        "10\n20\n30\n",
    );
}

#[test]
fn stage18_200_vec_get_after_growth() {
    assert_runtime(
        "vec-get-after-growth",
        r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    v.push(4);
    v.push(5);
    println!("{}", v.get(4));
    0
}
"#,
        "5\n",
    );
}

#[test]
fn stage18_200_vec_get_oob_panics() {
    let code = r#"
fn main() -> i32 {
    let mut v: Vec<i32> = Vec::new();
    v.push(10);
    let x = v.get(5);
    println!("{}", x);
    0
}
"#;
    let (_stdout, exit) = run_program(code);
    assert_ne!(exit, 0, "expected OOB panic (exit != 0), got exit {}", exit);
}
