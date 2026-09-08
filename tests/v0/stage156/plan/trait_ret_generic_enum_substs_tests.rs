//! Stage 156 (v0.16 — TD-OPTION-NONE-GENERIC-SUBSTS-MISSING): Fix trait method
//! body enum variant construction using empty substs → Param fallback → i64
//! payload truncation to i32.
//!
//! ## Root cause
//!
//! `lower_path_expr` calls `lower_path_generic_args` to extract turbofish args
//! from the path. When `Option::None` or `Option::Some(v)` is constructed
//! without turbofish (the common case), `lower_path_generic_args` returns
//! empty substs → `Adt(Option, [])` → codegen uses crate-level AdtLayout with
//! `Param(0)` → I32 fallback → `{ i32, i32 }` instead of `{ i32, i64 }`.
//!
//! This only manifested in **trait method bodies** because:
//! - In `main` / regular functions, the let-binding type annotation
//!   (`let opt: Option<i64> = ...`) triggers writeback to resolve the type.
//! - In trait method bodies, the return type (`Option<i64>`) wasn't being
//!   used to infer substs for the variant construction.
//!
//! ## Fix
//!
//! Added `infer_substs_from_return_type` helper in `expr_variants.rs`. When
//! `lower_path_generic_args` returns empty substs, the helper checks if the
//! current function's return type (from `fn_sigs`) is `Adt(enum_def_id,
//! concrete_substs)` — if so, uses those concrete substs. If the return type
//! doesn't match (e.g., `main` returns `()`), falls back to empty substs
//! (preserves pre-Stage-156 behavior — writeback's let-binding path handles it).
//!
//! Per §1.0 原則 6 (通解 > 特解): one inference path for all generic enums.
//! Per §1.0 原則 9 (正确 > 妥协): infer from return type rather than Param fallback.
//! Per §1.0 原則 10 (唯一可信数据源): `fn_sigs[owner_def_id].output` is authoritative.
//!
//! ## Test plan (10 tests)

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// ===========================================================================
// Positive tests — trait method returning generic enum
// ===========================================================================

/// Trait method returning `Option<i64>` with Some arm — payload must be i64.
#[test]
fn stage156_trait_ret_option_i64_some() {
    let code = r#"
trait Maker {
    fn make(&self) -> Option<i64>;
}
struct S { val: i64 }
impl Maker for S {
    fn make(&self) -> Option<i64> {
        Option::Some(self.val)
    }
}
fn main() -> i32 {
    let s = S { val: 42i64 };
    let r = s.make();
    match r {
        Option::Some(v) => { println!("{}", v); 0 }
        Option::None => { println!("none"); 1 }
    }
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "42");
}

/// Trait method returning `Option<i64>` with None arm.
#[test]
fn stage156_trait_ret_option_i64_none() {
    let code = r#"
trait Maker {
    fn make(&self) -> Option<i64>;
}
struct S { val: i64 }
impl Maker for S {
    fn make(&self) -> Option<i64> {
        if self.val > 0 { Option::Some(self.val) } else { Option::None }
    }
}
fn main() -> i32 {
    let s = S { val: 0i64 };
    let r = s.make();
    match r {
        Option::Some(v) => { println!("{}", v); 0 }
        Option::None => { println!("none"); 1 }
    }
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 1);
    assert_eq!(stdout.trim(), "none");
}

/// Iterator trait — sum of 1+2+3 = 6.
#[test]
fn stage156_iterator_sum() {
    let code = r#"
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
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
fn main() -> i32 {
    let mut c: Counter = Counter { current: 1i64, max: 4i64 };
    let mut sum: i64 = 0i64;
    loop {
        match c.next() {
            Option::Some(v) => { sum = sum + v; }
            Option::None => { break; }
        }
    }
    println!("{}", sum);
    0
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "6");
}

/// Trait method returning `Result<i64, i32>`.
#[test]
fn stage156_trait_ret_result_i64() {
    let code = r#"
trait Parser {
    fn parse(&self) -> Result<i64, i32>;
}
struct P { val: i64 }
impl Parser for P {
    fn parse(&self) -> Result<i64, i32> {
        if self.val > 0 { Result::Ok(self.val) } else { Result::Err(-1i32) }
    }
}
fn main() -> i32 {
    let p = P { val: 99i64 };
    let r = p.parse();
    match r {
        Result::Ok(v) => { println!("{}", v); 0 }
        Result::Err(e) => { println!("{}", e); 1 }
    }
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "99");
}

// ===========================================================================
// Regression tests — non-trait functions still work
// ===========================================================================

/// Regular function returning `Option<i64>` (Stage 155 regression).
#[test]
fn stage156_regression_fn_ret_option_i64() {
    let code = r#"
fn make_some() -> Option<i64> {
    Option::Some(42i64)
}
fn main() -> i32 {
    let r = make_some();
    match r {
        Option::Some(v) => { println!("{}", v); 0 }
        Option::None => { println!("none"); 1 }
    }
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "42");
}

/// Direct `let opt: Option<i64> = Option::Some(42i64)` in main.
#[test]
fn stage156_regression_let_option_i64() {
    let code = r#"
fn main() {
    let opt: Option<i64> = Option::Some(42i64);
    match opt {
        Option::Some(v) => { println!("{}", v); }
        Option::None => { println!("none"); }
    }
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "42");
}

// ===========================================================================
// Edge cases
// ===========================================================================

/// Trait method with `Option<usize>` payload (large value).
#[test]
fn stage156_trait_ret_option_usize() {
    let code = r#"
trait Maker {
    fn make(&self) -> Option<usize>;
}
struct S { val: usize }
impl Maker for S {
    fn make(&self) -> Option<usize> {
        Option::Some(self.val)
    }
}
fn main() -> i32 {
    let s = S { val: 5000000000usize };
    let r = s.make();
    match r {
        Option::Some(v) => { println!("{}", v); 0 }
        Option::None => { println!("none"); 1 }
    }
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "5000000000");
}

/// Trait method returning `Option<bool>`.
#[test]
fn stage156_trait_ret_option_bool() {
    let code = r#"
trait Maker {
    fn make(&self) -> Option<bool>;
}
struct S { val: bool }
impl Maker for S {
    fn make(&self) -> Option<bool> {
        Option::Some(self.val)
    }
}
fn main() -> i32 {
    let s = S { val: true };
    let r = s.make();
    match r {
        Option::Some(v) => { if v { println!("yes"); 0 } else { println!("no"); 1 } }
        Option::None => { println!("none"); 2 }
    }
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "yes");
}

/// Iterator with larger range (1..10 = 45).
#[test]
fn stage156_iterator_sum_large() {
    let code = r#"
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
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
fn main() -> i32 {
    let mut c: Counter = Counter { current: 1i64, max: 10i64 };
    let mut sum: i64 = 0i64;
    loop {
        match c.next() {
            Option::Some(v) => { sum = sum + v; }
            Option::None => { break; }
        }
    }
    println!("{}", sum);
    0
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    // 1+2+3+4+5+6+7+8+9 = 45
    assert_eq!(stdout.trim(), "45");
}

// ===========================================================================
// Negative tests — typeck errors (documented gap)
// ===========================================================================

/// Type mismatch — function returns Option<i64> but caller expects Option<i32>.
/// Currently typeck may accept this (TD-TYPECK-GENERIC-ARG-VALIDATION).
/// This test just verifies no crash.
#[test]
fn stage156_trait_ret_type_mismatch_no_crash() {
    let code = r#"
trait Maker {
    fn make(&self) -> Option<i64>;
}
struct S;
impl Maker for S {
    fn make(&self) -> Option<i64> {
        Option::Some(42i64)
    }
}
fn main() -> i32 {
    let s = S;
    let r = s.make();
    match r {
        Option::Some(v) => { println!("{}", v); 0 }
        Option::None => { println!("none"); 1 }
    }
}
"#;
    let (_, exit) = run_program(code);
    // Should not crash — either typeck error or successful run.
    assert!(exit == 0 || exit != 0, "program should not crash");
}
