//! Stage 136 (v0.15 — TD-CODEGEN-OPT-LEVELS): Optimization level tests.

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

#[test]
fn stage136_opt_level_default() {
    let code = r#"
fn main() {
    let x = 5;
    let y = x + 3;
    println!("{}", y);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Default opt level should work");
    assert_eq!(stdout.trim(), "8");
}

#[test]
fn stage136_opt_level_0() {
    let code = r#"
fn main() {
    let x = 10;
    let y = x * 2;
    println!("{}", y);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "O0 should work");
    assert_eq!(stdout.trim(), "20");
}

#[test]
fn stage136_opt_level_2() {
    let code = r#"
fn main() {
    let x = 7;
    let y = x - 2;
    println!("{}", y);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "O2 should work");
    assert_eq!(stdout.trim(), "5");
}

#[test]
fn stage136_extern_block_regression() {
    let code = r#"
extern "C" fn my_c_func(x: i32) -> i32 {
    x + 1
}

fn main() {
    let x = my_c_func(42);
    println!("{}", x);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "extern fn regression");
    assert_eq!(stdout.trim(), "43");
}
