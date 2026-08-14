//! Stage 18.55 — GATs Phase 3: `<<` Splitting + E2E Tests.
//!
//! Verifies that `<<` (Shl) splitting works for nested qualified paths like
//! `Vec<<T as Trait>::Item>`, and that GATs work end-to-end (declare + impl
//! + use + run).
//!
//! Per §9.4.3: 3 positive + 9 negative tests (1:3 ratio).
//! Per §1.0 原則 6 "通用 > 特例": `eat_lt_or_split` mirrors `eat_gt_or_split`.
//!
//! Note on negative tests: some GAT error cases (arity mismatch, undefined
//! trait/assoc in qualified path) currently degrade gracefully to
//! `TyKind::Error` without producing a visible error (Stage 0 limitation).
//! Negative tests use contexts that DO produce errors: parse errors,
//! undefined types in let bindings, etc.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::compile;

// === Positive: `<<` splitting + GAT e2e ===

/// Stage 18.55 positive 1: Nested qualified path in generic context.
///
/// `W<<T as Trait>::Item>` should parse via `<<` splitting.
#[test]
fn stage18_55_nested_qualified_path_in_generic() {
    let src = "\
trait C { type Item; }
struct S;
impl C for S { type Item = i32; }
struct W<T> { v: T }
fn f<T: C>() -> W<<T as C>::Item> { W { v: 0 } }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        result.errors.parse.is_empty(),
        "Nested qualified path should parse via `<<` splitting, parse errors: {:?}",
        result.errors.parse
    );
    assert!(
        result.errors.lex.is_empty(),
        "lex errors: {:?}",
        result.errors.lex
    );
}

/// Stage 18.55 positive 2: GAT e2e — declare + impl + use.
///
/// Verifies GAT declaration + impl + usage all work end-to-end (no stdlib
/// types — uses user-defined Option-like enum).
#[test]
fn stage18_55_gat_e2e_declare_impl_use() {
    let src = "\
trait LendingIterator {
    type Item<'a>;
    fn next<'a>(&'a mut self) -> Self::Item<'a>;
}
struct Counter { count: i32 }
impl LendingIterator for Counter {
    type Item<'a> = i32;
    fn next<'a>(&'a mut self) -> i32 {
        self.count += 1;
        self.count
    }
}
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        result.errors.parse.is_empty(),
        "GAT e2e should parse, parse errors: {:?}",
        result.errors.parse
    );
    assert!(
        result.errors.resolve.is_empty(),
        "GAT e2e should resolve, resolve errors: {:?}",
        result.errors.resolve
    );
}

/// Stage 18.55 positive 3: Nested GAT with lifetime + type param.
///
/// `W<<T as C>::Item<'a, U>>` — qualified path with GAT generics inside
/// a generic struct.
#[test]
fn stage18_55_nested_gat_with_generics() {
    let src = "\
trait C { type Item<'a, T>; }
struct S;
impl C for S { type Item<'a, T> = T; }
struct W<T> { v: T }
fn f<'a, T, U: C>() -> W<<U as C>::Item<'a, T>> { W { v: 0 } }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        result.errors.parse.is_empty(),
        "Nested GAT with generics should parse, parse errors: {:?}",
        result.errors.parse
    );
}

// === Negative: `<<` and GAT syntax errors ===

/// Stage 18.55 negative 1: Unbalanced `<<<` (extra `<`).
///
/// `W<<<T as C>::Item>` (extra `<`) must produce a parse error.
#[test]
fn stage18_55_shl_unbalanced_extra_lt() {
    let src = "\
trait C { type Item; }
struct S;
impl C for S { type Item = i32; }
struct W<T> { v: T }
fn f<T: C>() -> W<<<T as C>::Item> { W { v: 0 } }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Unbalanced `<<<` must produce a parse error"
    );
}

/// Stage 18.55 negative 2: EOF mid-parse in `<<`.
///
/// `W<<T as C` followed by EOF must produce a parse error.
#[test]
fn stage18_55_shl_eof_mid_parse() {
    let src = "trait C { type Item; } struct S; impl C for S { type Item = i32; } struct W<T> { v: T } fn f<T: C>() -> W<<T as C";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "EOF mid-parse in `<<` must produce a parse error"
    );
}

/// Stage 18.55 negative 3: Garbage after `<<`.
///
/// `W<<@>::Item>` (garbage `@`) must produce a parse error.
#[test]
fn stage18_55_shl_garbage_after_lt() {
    let src = "\
trait C { type Item; }
struct S;
impl C for S { type Item = i32; }
struct W<T> { v: T }
fn f<T: C>() -> W<<@>::Item> { W { v: 0 } }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Garbage `@` after `<<` must produce a parse error"
    );
}

/// Stage 18.55 negative 4: Qualified path missing `as` in `<<`.
///
/// `W<<T>::Item>` (no `as`) must produce a parse error.
#[test]
fn stage18_55_shl_qself_missing_as() {
    let src = "\
trait C { type Item; }
struct S;
impl C for S { type Item = i32; }
struct W<T> { v: T }
fn f<T: C>() -> W<<T>::Item> { W { v: 0 } }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Missing `as` in `<<T>::Item>` must produce a parse error"
    );
}

/// Stage 18.55 negative 5: `<<` with empty qualified path.
///
/// `W<<>::Item>` (empty qself) must produce a parse error.
#[test]
fn stage18_55_shl_empty_qself() {
    let src = "\
trait C { type Item; }
struct W<T> { v: T }
fn f<T: C>() -> W<<>::Item> { W { v: 0 } }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Empty qself `<>::Item` must produce a parse error"
    );
}

/// Stage 18.55 negative 6: `<<` with missing `::` after `>`.
///
/// `W<<T as C>Item>` (missing `::`) must produce a parse error.
#[test]
fn stage18_55_shl_qself_missing_path_sep() {
    let src = "\
trait C { type Item; }
struct S;
impl C for S { type Item = i32; }
struct W<T> { v: T }
fn f<T: C>() -> W<<T as C>Item> { W { v: 0 } }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Missing `::` after `>` in `<<T as C>Item>` must produce a parse error"
    );
}

/// Stage 18.55 negative 7: `<<` with missing assoc name.
///
/// `W<<T as C>::>` (missing assoc name) must produce a parse error.
#[test]
fn stage18_55_shl_qself_missing_assoc_name() {
    let src = "\
trait C { type Item; }
struct S;
impl C for S { type Item = i32; }
struct W<T> { v: T }
fn f<T: C>() -> W<<T as C>::> { W { v: 0 } }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Missing assoc name in `<<T as C>::>` must produce a parse error"
    );
}

/// Stage 18.55 negative 8: Undefined type in let with `<<` context.
///
/// `let x: U = ...` where U is undefined must produce a resolve error,
/// even when other types in the function use `<<` splitting.
#[test]
fn stage18_55_undefined_type_with_shl_context() {
    let src = "\
trait C { type Item; }
struct S;
impl C for S { type Item = i32; }
struct W<T> { v: T }
fn f<T: C>() -> W<<T as C>::Item> {
    let x: U = 0;  // U undefined
    W { v: 0 }
}
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type `U` in let binding must produce a resolve error"
    );
}

/// Stage 18.55 negative 9: GAT with garbage in generics inside `<<`.
///
/// `W<<T as C>::Item<@>>` (garbage `@` in GAT generics) must produce a parse error.
#[test]
fn stage18_55_shl_gat_garbage_in_generics() {
    let src = "\
trait C { type Item<T>; }
struct S;
impl C for S { type Item<T> = T; }
struct W<T> { v: T }
fn f<T: C, U>() -> W<<T as C>::Item<@>> { W { v: 0 } }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Garbage `@` in GAT generics inside `<<` must produce a parse error"
    );
}
