//! Stage 149 (v0.15 — TD-TRAIT-METHOD-GENERIC-RET-SKIP): Fix trait impl
//! methods returning generic enums (Option<T>) being skipped by codegen.
//!
//! ## Root cause
//!
//! `mir_body_contains_param_type` checked `Aggregate` substs and field_tys
//! for Param types. For trait impl methods returning `Option<T>`, the
//! Aggregate's substs contained `Param(0)` (the `T` from `Option<T>`),
//! causing the function to be falsely classified as "generic" and skipped
//! by `codegen_from_mir`'s prelude generic skip rule.
//!
//! ## Fix
//!
//! `statement_contains_param` now only checks OPERANDS (actual values),
//! not type metadata (substs/field_tys). The Param in type metadata is a
//! typeck limitation, not an indication that the function is generic.
//!
//! ## Test plan
//!
//! - Trait method returning Option with if/else: 5 tests
//! - Trait method returning Option without if/else: 3 tests
//! - Iterator trait (basic): 4 tests
//! - Regression: 3 tests
//! - Total: 15 tests

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// ===========================================================================
// Trait method returning Option with if/else
// ===========================================================================

#[test]
fn stage149_trait_method_opt_if_else() {
    let code = r#"
trait Foo {
    fn bar(&self) -> Option<i64>;
}

struct S { val: i64 }

impl Foo for S {
    fn bar(&self) -> Option<i64> {
        if self.val > 0i64 {
            Option::Some(self.val)
        } else {
            Option::Some(0i64)
        }
    }
}

fn main() {
    let s: S = S { val: 42i64 };
    let opt: Option<i64> = s.bar();
    println!("{}", opt.is_some() as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "trait method opt if/else should compile");
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage149_trait_method_opt_if_else_none() {
    let code = r#"
trait Foo {
    fn bar(&self) -> Option<i64>;
}

struct S { val: i64 }

impl Foo for S {
    fn bar(&self) -> Option<i64> {
        if self.val > 0i64 {
            Option::Some(self.val)
        } else {
            Option::None
        }
    }
}

fn main() {
    let s1: S = S { val: 42i64 };
    let s2: S = S { val: 0i64 };
    println!("{} {}", s1.bar().is_some() as i64, s2.bar().is_some() as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "trait method opt if/else none should compile");
    assert_eq!(stdout.trim(), "1 0");
}

#[test]
fn stage149_trait_method_opt_if_elseif() {
    let code = r#"
trait Foo {
    fn bar(&self) -> Option<i64>;
}

struct S { val: i64 }

impl Foo for S {
    fn bar(&self) -> Option<i64> {
        if self.val > 10i64 {
            Option::Some(self.val)
        } else if self.val > 0i64 {
            Option::Some(1i64)
        } else {
            Option::None
        }
    }
}

fn main() {
    let s1: S = S { val: 42i64 };
    let s2: S = S { val: 5i64 };
    let s3: S = S { val: 0i64 };
    println!("{} {} {}", s1.bar().is_some() as i64, s2.bar().is_some() as i64, s3.bar().is_some() as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "trait method opt if/elseif should compile");
    assert_eq!(stdout.trim(), "1 1 0");
}

#[test]
fn stage149_trait_method_opt_mutation() {
    let code = r#"
trait Foo {
    fn next(&mut self) -> Option<i64>;
}

struct Counter { current: i64, max: i64 }

impl Foo for Counter {
    fn next(&mut self) -> Option<i64> {
        if self.current < self.max {
            let v: i64 = self.current;
            self.current = self.current + 1i64;
            Option::Some(v)
        } else {
            Option::None
        }
    }
}

fn main() {
    let mut c: Counter = Counter { current: 1i64, max: 5i64 };
    let mut count: i64 = 0i64;
    loop {
        let opt: Option<i64> = c.next();
        if opt.is_some() {
            count = count + 1i64;
        } else {
            break;
        }
    }
    println!("{}", count);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "trait method opt mutation should compile");
    assert_eq!(stdout.trim(), "4");
}

#[test]
fn stage149_trait_method_opt_return_i32() {
    let code = r#"
trait Foo {
    fn bar(&self) -> Option<i32>;
}

struct S { val: i32 }

impl Foo for S {
    fn bar(&self) -> Option<i32> {
        if self.val > 0i32 {
            Option::Some(self.val)
        } else {
            Option::None
        }
    }
}

fn main() {
    let s: S = S { val: 42i32 };
    let opt: Option<i32> = s.bar();
    println!("{}", opt.is_some() as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "trait method opt return i32 should compile");
    assert_eq!(stdout.trim(), "1");
}

// ===========================================================================
// Trait method returning Option without if/else
// ===========================================================================

#[test]
fn stage149_trait_method_opt_simple_some() {
    let code = r#"
trait Foo {
    fn bar(&self) -> Option<i64>;
}

struct S { val: i64 }

impl Foo for S {
    fn bar(&self) -> Option<i64> {
        Option::Some(self.val)
    }
}

fn main() {
    let s: S = S { val: 42i64 };
    let opt: Option<i64> = s.bar();
    println!("{}", opt.is_some() as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "trait method opt simple some should compile");
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage149_trait_method_opt_simple_none() {
    let code = r#"
trait Foo {
    fn bar(&self) -> Option<i64>;
}

struct S;

impl Foo for S {
    fn bar(&self) -> Option<i64> {
        Option::None
    }
}

fn main() {
    let s: S = S;
    let opt: Option<i64> = s.bar();
    println!("{}", opt.is_some() as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "trait method opt simple none should compile");
    assert_eq!(stdout.trim(), "0");
}

#[test]
fn stage149_trait_method_opt_conditional() {
    let code = r#"
trait Foo {
    fn bar(&self) -> Option<i64>;
}

struct S { val: i64 }

impl Foo for S {
    fn bar(&self) -> Option<i64> {
        if self.val > 0i64 { Option::Some(self.val) } else { Option::None }
    }
}

fn main() {
    let s1: S = S { val: 42i64 };
    let s2: S = S { val: 0i64 };
    println!("{} {}", s1.bar().is_some() as i64, s2.bar().is_some() as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "trait method opt conditional should compile");
    assert_eq!(stdout.trim(), "1 0");
}

// ===========================================================================
// Iterator trait (basic — uses Option<Self::Item>)
// ===========================================================================

#[test]
fn stage149_iterator_counter_count() {
    let code = r#"
struct Counter { current: i64, max: i64 }

impl Iterator for Counter {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        if self.current < self.max {
            let v: i64 = self.current;
            self.current = self.current + 1i64;
            Option::Some(v)
        } else {
            Option::None
        }
    }
}

fn main() {
    let mut c: Counter = Counter { current: 1i64, max: 5i64 };
    let mut count: i64 = 0i64;
    loop {
        let opt: Option<i64> = c.next();
        if opt.is_some() { count = count + 1i64; } else { break; }
    }
    println!("{}", count);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Iterator counter count should compile");
    assert_eq!(stdout.trim(), "4");
}

#[test]
fn stage149_iterator_empty_iter() {
    let code = r#"
struct EmptyIter;

impl Iterator for EmptyIter {
    type Item = i64;
    fn next(&mut self) -> Option<i64> { Option::None }
}

fn main() {
    let mut it: EmptyIter = EmptyIter;
    println!("{}", it.next().is_some() as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Iterator empty should compile");
    assert_eq!(stdout.trim(), "0");
}

#[test]
fn stage149_iterator_once() {
    let code = r#"
struct Once { value: i64, done: bool }

impl Iterator for Once {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        if !self.done {
            self.done = true;
            Option::Some(self.value)
        } else {
            Option::None
        }
    }
}

fn main() {
    let mut it: Once = Once { value: 42i64, done: false };
    println!("{}", it.next().is_some() as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Iterator once should compile");
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage149_iterator_range() {
    let code = r#"
struct Range { current: i64, end: i64 }

impl Iterator for Range {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        if self.current < self.end {
            let v: i64 = self.current;
            self.current = self.current + 1i64;
            Option::Some(v)
        } else {
            Option::None
        }
    }
}

fn main() {
    let mut r: Range = Range { current: 0i64, end: 10i64 };
    let mut count: i64 = 0i64;
    loop {
        let opt: Option<i64> = r.next();
        if opt.is_some() { count = count + 1i64; } else { break; }
    }
    println!("{}", count);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Iterator range should compile");
    assert_eq!(stdout.trim(), "10");
}

// ===========================================================================
// Regression
// ===========================================================================

#[test]
fn stage149_regression_simple_trait() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i64;
}

struct English;

impl Greeter for English {
    fn greet(&self) -> i64 { 42i64 }
}

fn main() {
    let e: English = English;
    println!("{}", e.greet());
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "simple trait regression");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage149_regression_display() {
    let code = r#"
fn main() {
    let n: i64 = 42i64;
    println!("{}", n);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Display trait regression");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage149_regression_option_methods() {
    let code = r#"
fn main() {
    let some: Option<i64> = Option::Some(42i64);
    let none: Option<i64> = Option::None;
    println!("{} {}", some.is_some() as i64, none.is_some() as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Option methods regression");
    assert_eq!(stdout.trim(), "1 0");
}
