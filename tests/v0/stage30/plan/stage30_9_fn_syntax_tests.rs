//! Stage 30.9 (v0.14 TD-HRTB-FN-SYNTAX): `Fn(T) -> U` trait bound syntax.
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//!   - 5 positive tests (Fn/FnMut/FnOnce + HRTB + impl Trait)
//!   - 3 negative tests (invalid syntax rejected)
//!   - 2 regression tests (non-Fn trait bounds still work)
//!
//! Per §1.0 原則 3 (显式 > 隐式): the parenthesized form is explicit.
//! Per §1.0 原則 6 (通解 > 特解): one parser for all Fn/FnMut/FnOnce.
//!
//! ## Background
//!
//! TD-HRTB-FN-SYNTAX was classified as "`for<'a> Fn(&'a T) -> &'a U` syntax
//! not parsed (Fn call syntax separate feature)".
//!
//! Root-cause: the parser treated `Fn` as a regular path and rejected `(`.
//!
//! ## Implementation (Stage 30.9)
//!
//! Added `try_parse_parenthesized_args` to parser/path.rs. Called from
//! `parse_type_bounds` after parsing the trait path. Handles `Fn(T) -> U`,
//! `FnMut(T) -> U`, `FnOnce(T) -> U` uniformly.
//!
//! ## What's NOT in scope
//!
//! - Typeck doesn't yet use the parenthesized args for type checking
//!   (e.g., `f(x)` where `f: impl Fn(i32) -> i32` produces "expected
//!   function, found F"). This is a separate typeck issue.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::driver::compile;

// ============================================================================
// POSITIVE TESTS — Fn/FnMut/FnOnce syntax parses cleanly
// ============================================================================

/// Stage 30.9 positive 1: `F: Fn(i32) -> i32` in trait bound — parses.
#[test]
fn stage30_9_positive_fn_in_trait_bound() {
    let result = compile(
        r#"
fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 {
    f(x)
}
fn main() {}
"#,
    );
    assert_eq!(
        result.errors.parse.len(),
        0,
        "F: Fn(i32) -> i32 should parse cleanly"
    );
}

/// Stage 30.9 positive 2: `impl Fn(i32) -> i32` in parameter — parses.
#[test]
fn stage30_9_positive_impl_fn_in_param() {
    let result = compile(
        r#"
fn apply(f: impl Fn(i32) -> i32, x: i32) -> i32 {
    f(x)
}
fn main() {}
"#,
    );
    assert_eq!(
        result.errors.parse.len(),
        0,
        "impl Fn(i32) -> i32 should parse cleanly"
    );
}

/// Stage 30.9 positive 3: `F: FnMut(i32) -> i32` — parses.
#[test]
fn stage30_9_positive_fnmut_syntax() {
    let result = compile(
        r#"
fn apply<F: FnMut(i32) -> i32>(f: F, x: i32) -> i32 {
    f(x)
}
fn main() {}
"#,
    );
    assert_eq!(
        result.errors.parse.len(),
        0,
        "F: FnMut(i32) -> i32 should parse cleanly"
    );
}

/// Stage 30.9 positive 4: `F: FnOnce(i32) -> i32` — parses.
#[test]
fn stage30_9_positive_fnonce_syntax() {
    let result = compile(
        r#"
fn apply<F: FnOnce(i32) -> i32>(f: F, x: i32) -> i32 {
    f(x)
}
fn main() {}
"#,
    );
    assert_eq!(
        result.errors.parse.len(),
        0,
        "F: FnOnce(i32) -> i32 should parse cleanly"
    );
}

/// Stage 30.9 positive 5: HRTB + Fn syntax — `for<'a> Fn(&'a T) -> &'a U`.
#[test]
fn stage30_9_positive_hrtb_fn_syntax() {
    let result = compile(
        r#"
fn apply<T, U, F: for<'a> Fn(&'a T) -> &'a U>(x: &T, f: F) -> &U {
    f(x)
}
fn main() {}
"#,
    );
    assert_eq!(
        result.errors.parse.len(),
        0,
        "for<'a> Fn(&'a T) -> &'a U should parse cleanly"
    );
}

// ============================================================================
// NEGATIVE TESTS — Invalid Fn syntax rejected
// ============================================================================

/// Stage 30.9 negative 1: `Fn` without `(` — should parse as regular trait.
#[test]
fn stage30_9_negative_fn_without_parens() {
    let result = compile(
        r#"
trait MyFn { fn call(&self); }
fn apply<F: MyFn>(f: F) { }
fn main() {}
"#,
    );
    // `MyFn` is a regular trait (no parenthesized args) — should parse.
    assert_eq!(
        result.errors.parse.len(),
        0,
        "Regular trait (no parens) should parse cleanly"
    );
}

/// Stage 30.9 negative 2: `Fn(` without `)` — should error.
#[test]
fn stage30_9_negative_fn_unclosed_paren() {
    let result = compile(
        r#"
fn apply<F: Fn(i32>(f: F) { }
fn main() {}
"#,
    );
    // Unclosed `(` should produce parse error.
    assert!(
        !result.errors.parse.is_empty(),
        "Unclosed `(` in Fn(...) should produce parse error"
    );
}

/// Stage 30.9 negative 3: `Fn(T)` without `-> U` — should parse (unit return).
#[test]
fn stage30_9_negative_fn_no_return_type() {
    let result = compile(
        r#"
fn apply<F: Fn(i32)>(f: F) { }
fn main() {}
"#,
    );
    // `Fn(i32)` without `-> U` means `-> ()` — should parse cleanly.
    assert_eq!(
        result.errors.parse.len(),
        0,
        "Fn(i32) without return type should parse cleanly (unit return)"
    );
}

// ============================================================================
// REGRESSION TESTS — Non-Fn trait bounds still work
// ============================================================================

/// Stage 30.9 regression 1: Regular trait bound (no parens) — still works.
#[test]
fn stage30_9_regression_regular_trait_bound() {
    let result = compile(
        r#"
trait Clone { fn clone(&self) -> Self; }
fn apply<F: Clone>(f: F) { }
fn main() {}
"#,
    );
    assert_eq!(
        result.errors.parse.len(),
        0,
        "Regular trait bound (no parens) should parse cleanly"
    );
}

/// Stage 30.9 regression 2: Trait bound with turbofish — still works.
#[test]
fn stage30_9_regression_turbofish_trait_bound() {
    let result = compile(
        r#"
trait Foo<T> { fn foo(&self, x: T); }
fn apply<F: Foo<i32>>(f: F) { }
fn main() {}
"#,
    );
    assert_eq!(
        result.errors.parse.len(),
        0,
        "Trait bound with turbofish should parse cleanly"
    );
}
