//! Stage 134 (v0.14 — TD-TRACE-MACROS-MACRO): trace_macros! tests.

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

#[test]
fn stage134_trace_macros_true() {
    let code = r#"
fn main() {
    trace_macros!(true);
    let x = 42;
    println!("{}", x);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "trace_macros!(true) should be no-op");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage134_trace_macros_false() {
    let code = r#"
fn main() {
    trace_macros!(false);
    let x = 99;
    println!("{}", x);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "trace_macros!(false) should be no-op");
    assert_eq!(stdout.trim(), "99");
}

#[test]
fn stage134_regression_matches_macro() {
    let code = r#"
fn main() {
    let x = 5;
    let is_five = matches!(x, 5);
    println!("{}", is_five);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "matches! regression");
    assert_eq!(stdout.trim(), "true");
}
