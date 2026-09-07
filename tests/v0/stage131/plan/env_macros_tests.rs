//! Stage 131 (v0.14 — TD-ENV-MACROS): env!/option_env!/include_str! tests.
//!
//! Tests for compile-time env var and file reading macros.

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// =====================================================================
// POSITIVE TESTS
// =====================================================================

#[test]
fn stage131_env_macro_basic() {
    let code = r#"
fn main() {
    let v = env!("HOME");
    println!("{}", v);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "env! should work");
    // HOME env var should be set in test environment
    assert!(!stdout.trim().is_empty());
}

#[test]
fn stage131_option_env_macro_set() {
    let code = r#"
fn main() {
    let v = option_env!("HOME");
    println!("{}", v);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "option_env! should work");
    assert!(!stdout.trim().is_empty());
}

#[test]
fn stage131_option_env_macro_unset() {
    let code = r#"
fn main() {
    let v = option_env!("LANDIN_NONEXISTENT_VAR_12345");
    println!("{}", v);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "option_env! for unset var should work");
    assert_eq!(stdout.trim(), "");
}

// =====================================================================
// REGRESSION TESTS
// =====================================================================

#[test]
fn stage131_regression_stringify_macro() {
    let code = r#"
fn main() {
    let s = stringify!(hello world);
    println!("{}", s);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "stringify! regression");
    assert_eq!(stdout.trim(), "hello world");
}

#[test]
fn stage131_regression_concat_macro() {
    let code = r#"
fn main() {
    let s = concat!("hello", " ", "world");
    println!("{}", s);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "concat! regression");
    assert_eq!(stdout.trim(), "hello world");
}
