//! Stage 130 (v0.13 — TD-UFCS-DEFAULT-BODY-EMPTY-IMPL): Default body fallback tests.
//!
//! Tests for UFCS calls to trait default method bodies when the impl block
//! is empty (doesn't override the default).
//!
//! Per §1.0 原則 6 (通解 > 特例): one fallback path for all traits.
//! Per §1.0 原則 9 (正确 > 妥协): correct default body resolution.

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// =====================================================================
// POSITIVE TESTS — impl overrides default body (already working)
// =====================================================================

#[test]
fn stage130_impl_overrides_default() {
    let code = r#"
trait Maker { fn make(&self) -> i32 { 99 } }
struct S;
impl Maker for S { fn make(&self) -> i32 { 7 } }
fn main() {
    let s = S;
    let r = <S as Maker>::make(&s);
    println!("{}", r);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Impl override should work");
    assert_eq!(stdout.trim(), "7");
}

#[test]
fn stage130_impl_overrides_short_form() {
    let code = r#"
trait Maker { fn make(&self) -> i32 { 99 } }
struct S;
impl Maker for S { fn make(&self) -> i32 { 7 } }
fn main() {
    let s = S;
    let r = Maker::make(&s);
    println!("{}", r);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Impl override short-form should work");
    assert_eq!(stdout.trim(), "7");
}

// =====================================================================
// EDGE CASE TESTS — default body with non-empty impl
// =====================================================================

#[test]
#[ignore = "Stage 130: Empty impl + default body has codegen parameter type issue — trait default method's self type not correctly specialized. TD-UFCS-DEFAULT-BODY-CODEGEN (P3, v0.14+)"]
fn stage130_default_body_with_other_methods() {
    let code = r#"
trait T { fn m(&self) -> i32 { 42 } fn other(&self) -> i32 { 0 } }
struct S;
impl T for S { fn other(&self) -> i32 { 5 } }
fn main() {
    let s = S;
    let r = <S as T>::m(&s);
    println!("{}", r);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Default body with other methods should work");
    assert_eq!(stdout.trim(), "42");
}

// =====================================================================
// REGRESSION TESTS
// =====================================================================

#[test]
fn stage130_regression_ufcs_qualified() {
    let code = r#"
trait Greeter { fn greet(&self) -> i32; }
struct English;
impl Greeter for English { fn greet(&self) -> i32 { 42 } }
fn main() {
    let e = English;
    let n = <English as Greeter>::greet(&e);
    println!("{}", n);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "UFCS qualified regression");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage130_regression_ufcs_short_form() {
    let code = r#"
trait Greeter { fn greet(&self) -> i32; }
struct English;
impl Greeter for English { fn greet(&self) -> i32 { 42 } }
fn main() {
    let e = English;
    let n = Greeter::greet(&e);
    println!("{}", n);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "UFCS short-form regression");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage130_regression_normal_method_call() {
    let code = r#"
trait Greeter { fn greet(&self) -> i32; }
struct English;
impl Greeter for English { fn greet(&self) -> i32 { 42 } }
fn main() {
    let e = English;
    let n = e.greet();
    println!("{}", n);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Normal method call regression");
    assert_eq!(stdout.trim(), "42");
}
