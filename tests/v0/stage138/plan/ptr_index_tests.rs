//! Stage 138 (v0.15 — TD-PTR-INDEX-CONST): *const T indexing tests.

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

#[test]
fn stage138_const_ptr_index() {
    let code = r#"
fn main() {
    let arr: *const i32 = 0 as *const i32;
    let _v = arr[0];
    println!("ok");
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "*const T indexing should compile");
    assert_eq!(stdout.trim(), "ok");
}

#[test]
fn stage138_mut_ptr_index_regression() {
    let code = r#"
fn main() {
    let arr: *mut i32 = 0 as *mut i32;
    let _v = arr[0];
    println!("ok");
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "*mut T indexing regression");
    assert_eq!(stdout.trim(), "ok");
}

#[test]
fn stage138_regression_array_index() {
    let code = r#"
fn main() {
    let arr = [1, 2, 3];
    println!("{}", arr[1]);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Array indexing regression");
    assert_eq!(stdout.trim(), "2");
}
