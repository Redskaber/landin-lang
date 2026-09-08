//! Stage 158 (v0.16 — TD-VTABLE-DEFAULT-BODY-MISSING-ENTRY): Fix dyn dispatch
//! of trait default body methods — vtable now includes default body entries.
//!
//! ## Root cause
//!
//! Vtable construction (`traits/resolver.rs`) only traversed impl items to
//! build `vtable_entries`. When an impl didn't override a trait method with
//! a default body (e.g., `impl Trait for S {}` with `trait Trait { fn f(&self) -> i32 { 42 } }`),
//! the vtable was empty — no entry for `f`. Dyn dispatch (`g.f()` where
//! `g: &dyn Trait`) couldn't find `f` in the vtable → `matched_call` was
//! `None` → codegen fell through to static dispatch → direct call with wrong
//! arg type (fat pointer `{ptr, ptr}` vs thin pointer `ptr`).
//!
//! ## Fix
//!
//! After building vtable entries from impl items, also add entries for trait
//! default body methods that the impl didn't override. For each trait method
//! with a body (not just a declaration), if the impl doesn't already have an
//! entry for that method name, add a vtable entry with `fn_name =
//! landin_{trait}_default_{method}`.
//!
//! Per §1.0 原則 6 (通解 > 特解): one loop for all trait default body methods.
//! Per §1.0 原則 9 (正确 > 妥协): add default body entries, don't leave vtable empty.
//! Per §1.0 原則 10 (唯一可信数据源): HIR trait declaration is authoritative.
//!
//! ## Test plan (10 tests)

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// ===========================================================================
// Positive tests — dyn dispatch of default body methods
// ===========================================================================

/// Dyn dispatch of a simple default body method.
#[test]
fn stage158_dyn_default_body_simple() {
    let code = r#"
trait GetX {
    fn get_x(&self) -> i32 { 42 }
}
struct S;
impl GetX for S {}
fn use_getter(g: &dyn GetX) -> i32 {
    g.get_x()
}
fn main() -> i32 {
    let s = S;
    use_getter(&s)
}
"#;
    let (_, exit) = run_program(code);
    assert_eq!(exit, 42, "dyn dispatch of default body should return 42");
}

/// Dyn dispatch of default body that accesses self field.
#[test]
fn stage158_dyn_default_body_field_access() {
    let code = r#"
trait GetX {
    fn get_x(&self) -> i32 { self.x }
}
struct Point { x: i32, y: i32 }
impl GetX for Point {}
fn use_getter(g: &dyn GetX) -> i32 {
    g.get_x()
}
fn main() -> i32 {
    let p = Point { x: 99, y: 1 };
    use_getter(&p)
}
"#;
    let (_, exit) = run_program(code);
    assert_eq!(exit, 99, "dyn dispatch field access should return 99");
}

/// Dyn dispatch with both default body and impl method.
#[test]
fn stage158_dyn_default_body_with_impl() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i32 { 42 }
    fn name(&self) -> i32;
}
struct English;
impl Greeter for English {
    fn name(&self) -> i32 { 7 }
}
fn use_greeter(g: &dyn Greeter) -> i32 {
    g.greet() + g.name()
}
fn main() -> i32 {
    let e = English;
    use_greeter(&e)
}
"#;
    let (_, exit) = run_program(code);
    assert_eq!(exit, 49, "greet() + name() should be 42 + 7 = 49");
}

/// Dyn dispatch with `&mut self` default body.
#[test]
fn stage158_dyn_default_body_mut() {
    let code = r#"
trait Counter {
    fn bump(&mut self) { self.val = self.val + 1; }
}
struct C { val: i32 }
impl Counter for C {}
fn use_counter(c: &mut dyn Counter) -> i32 {
    c.bump();
    c.bump();
    0
}
fn main() -> i32 {
    let mut c = C { val: 10 };
    use_counter(&mut c);
    c.val
}
"#;
    let (_, exit) = run_program(code);
    assert_eq!(exit, 12, "after 2 bumps via dyn, val should be 12");
}

// ===========================================================================
// Regression tests — static dispatch still works
// ===========================================================================

/// Static dispatch of default body (Stage 157 regression).
#[test]
fn stage158_regression_static_default_body() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i32 { 42 }
}
struct English;
impl Greeter for English {}
fn main() -> i32 {
    let e = English;
    e.greet()
}
"#;
    let (_, exit) = run_program(code);
    assert_eq!(exit, 42);
}

/// Static dispatch with field access (Stage 157 regression).
#[test]
fn stage158_regression_static_field_access() {
    let code = r#"
trait GetX {
    fn get_x(&self) -> i32 { self.x }
}
struct Point { x: i32, y: i32 }
impl GetX for Point {}
fn main() -> i32 {
    let p = Point { x: 5, y: 10 };
    p.get_x()
}
"#;
    let (_, exit) = run_program(code);
    assert_eq!(exit, 5);
}

// ===========================================================================
// Edge cases
// ===========================================================================

/// Multiple traits with default body via dyn dispatch.
#[test]
fn stage158_dyn_multiple_traits() {
    let code = r#"
trait A { fn fa(&self) -> i32 { 1 } }
trait B { fn fb(&self) -> i32 { 2 } }
struct S;
impl A for S {}
impl B for S {}
fn use_a(g: &dyn A) -> i32 { g.fa() }
fn use_b(g: &dyn B) -> i32 { g.fb() }
fn main() -> i32 {
    let s = S;
    use_a(&s) + use_b(&s)
}
"#;
    let (_, exit) = run_program(code);
    assert_eq!(exit, 3, "fa() + fb() should be 1 + 2 = 3");
}

/// Dyn dispatch with args (non-self args).
#[test]
fn stage158_dyn_default_body_with_args() {
    let code = r#"
trait Adder {
    fn add_one(&self, n: i32) -> i32 { n + 1 }
}
struct Calc;
impl Adder for Calc {}
fn use_adder(a: &dyn Adder, n: i32) -> i32 {
    a.add_one(n)
}
fn main() -> i32 {
    let c = Calc;
    use_adder(&c, 41)
}
"#;
    let (_, exit) = run_program(code);
    assert_eq!(exit, 42, "add_one(41) via dyn should return 42");
}

/// Dyn dispatch where impl overrides default body.
#[test]
fn stage158_dyn_override_default() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i32 { 42 }
}
struct Spanish;
impl Greeter for Spanish {
    fn greet(&self) -> i32 { 99 }
}
fn use_greeter(g: &dyn Greeter) -> i32 {
    g.greet()
}
fn main() -> i32 {
    let s = Spanish;
    use_greeter(&s)
}
"#;
    let (_, exit) = run_program(code);
    assert_eq!(exit, 99, "impl override should take priority over default");
}

/// Let-binding dyn coercion + default body.
#[test]
fn stage158_dyn_let_default_body() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i32 { 42 }
}
struct English;
impl Greeter for English {}
fn main() -> i32 {
    let e = English;
    let g: &dyn Greeter = &e;
    g.greet()
}
"#;
    let (_, exit) = run_program(code);
    assert_eq!(exit, 42, "let-binding dyn + default body should return 42");
}
