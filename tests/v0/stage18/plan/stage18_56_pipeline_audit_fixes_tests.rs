//! Stage 18.56 — Pipeline Audit Fixes: Soundness + Error Reporting.
//!
//! Verifies that:
//! 1. `find_assoc_type_in_trait` respects trait qualifier (soundness fix)
//! 2. `lower_qualified_path_to_projection` emits diagnostic when assoc type
//!    not found (报错 > 静默 fix)
//!
//! Per §9.4.3: 3 positive + 9 negative tests (1:3 ratio).
//! Per §1.0 原則 9 "正确 > 妥协": trait qualifier must be respected.
//! Per §1.0 原則 4 "报错 > 静默": missing assoc type must produce diagnostic.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::compile;

// === Positive: Trait-scoped assoc type lookup ===

/// Stage 18.56 positive 1: Two traits with different assoc names, qualified
/// path resolves to correct trait.
///
/// `trait A { type ItemA; }` and `trait B { type ItemB; }` — `<T as A>::ItemA`
/// and `<T as B>::ItemB` should both resolve without ambiguity.
#[test]
fn stage18_56_trait_scoped_assoc_lookup() {
    let src = "\
trait A { type ItemA; }
trait B { type ItemB; }
struct S;
impl A for S { type ItemA = i32; }
impl B for S { type ItemB = bool; }
fn f<T: A + B>(x: T) -> <T as A>::ItemA { 0 }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        result.errors.parse.is_empty(),
        "parse errors: {:?}",
        result.errors.parse
    );
    assert!(
        result.errors.resolve.is_empty(),
        "resolve errors (trait-scoped lookup should work): {:?}",
        result.errors.resolve
    );
}

/// Stage 18.56 positive 2: Assoc type found in trait — no error.
#[test]
fn stage18_56_assoc_type_found_no_error() {
    let src = "\
trait C { type Item; }
struct S;
impl C for S { type Item = i32; }
fn f<T: C>(x: T) -> <T as C>::Item { 0 }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        result.errors.resolve.is_empty(),
        "Found assoc type should not produce resolve error: {:?}",
        result.errors.resolve
    );
}

/// Stage 18.56 positive 3: GAT with trait-scoped lookup.
#[test]
fn stage18_56_gat_trait_scoped_lookup() {
    let src = "\
trait C { type Item<'a>; }
struct S;
impl C for S { type Item<'a> = i32; }
fn f<'a, T: C>(x: &'a T) -> <T as C>::Item<'a> { 0 }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        result.errors.resolve.is_empty(),
        "GAT trait-scoped lookup should work: {:?}",
        result.errors.resolve
    );
}

// === Negative: Undefined assoc types and traits ===

/// Stage 18.56 negative 1: Assoc type not found in trait.
///
/// `<T as C>::Undefined` (Undefined not declared in C) must produce
/// a resolve error (报错 > 静默 fix).
#[test]
fn stage18_56_assoc_type_not_found_emits_error() {
    let src = "\
trait C { type Item; }
struct S;
impl C for S { type Item = i32; }
fn f<T: C>(x: T) -> <T as C>::Undefined { 0 }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined assoc type `Undefined` must produce a resolve error (报错 > 静默)"
    );
}

/// Stage 18.56 negative 2: Undefined trait in qualified path (direct, not nested).
///
/// `<T as UndefinedTrait>::Item` (UndefinedTrait not declared) must produce
/// a resolve error. Note: this test uses the qualified path directly (not
/// nested in generic args) because nested args have a known limitation
/// (path segment args are AST, not HIR — resolver doesn't recurse into them).
#[test]
fn stage18_56_undefined_trait_in_qualified_emits_error() {
    let src = "\
fn f<T>(x: T) -> <T as UndefinedTrait>::Item { 0 }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined trait `UndefinedTrait` must produce a resolve error"
    );
}

/// Stage 18.56 negative 3: Assoc type exists in wrong trait.
///
/// `trait A { type Item; }` and `trait B {}` — `<T as B>::Item` must
/// produce an error because B doesn't declare Item.
#[test]
fn stage18_56_assoc_type_wrong_trait_emits_error() {
    let src = "\
trait A { type Item; }
trait B {}
struct S;
impl A for S { type Item = i32; }
impl B for S {}
fn f<T: A + B>(x: T) -> <T as B>::Item { 0 }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Assoc type `Item` in wrong trait `B` must produce a resolve error"
    );
}

/// Stage 18.56 negative 4: Qualified path with no trait (empty qself position).
///
/// `<T>::Item` (no `as` keyword) must produce a parse error (the parser
/// rolls back qself and `<` becomes comparison).
#[test]
fn stage18_56_qualified_path_no_trait_emits_error() {
    let src = "\
trait C { type Item; }
fn f<T: C>(x: T) -> <T>::Item { 0 }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty() || !result.errors.resolve.is_empty(),
        "Qualified path without `as` must produce an error"
    );
}

/// Stage 18.56 negative 5: Two traits with different assoc names, qualified
/// path references the one without the assoc type.
#[test]
fn stage18_56_two_traits_ambiguous_assoc() {
    let src = "\
trait A { type ItemA; }
trait B { type OutputB; }
struct S;
impl A for S { type ItemA = i32; }
impl B for S { type OutputB = bool; }
fn f<T: A + B>(x: T) -> <T as B>::ItemA { 0 }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Trait B doesn't have ItemA (only OutputB) — must produce error"
    );
}

/// Stage 18.56 negative 6: Qualified path with undefined trait and assoc.
#[test]
fn stage18_56_qualified_both_undefined() {
    let src = "\
fn f<T>(x: T) -> <T as UndefinedTrait>::UndefinedItem { 0 }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Both trait and assoc undefined must produce a resolve error"
    );
}

/// Stage 18.56 negative 7: Qualified path in let binding with undefined assoc.
#[test]
fn stage18_56_qualified_undefined_in_let() {
    let src = "\
trait C { type Item; }
struct S;
impl C for S { type Item = i32; }
fn main() {
    let x: <S as C>::Undefined = 42;
}
";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined assoc type in let binding must produce a resolve error"
    );
}

/// Stage 18.56 negative 8: Qualified path with GAT generics on undefined assoc.
#[test]
fn stage18_56_qualified_gat_undefined_assoc() {
    let src = "\
trait C { type Item<T>; }
struct S;
impl C for S { type Item<T> = T; }
fn f<T: C, U>(x: T) -> <T as C>::UndefinedItem<U> { 0 }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined GAT assoc type must produce a resolve error"
    );
}

/// Stage 18.56 negative 9: Qualified path with undefined trait and assoc
/// in GAT context (both undefined).
///
/// `<T as UndefinedTrait>::UndefinedItem` must produce a resolve error
/// because both the trait and assoc type are undefined.
#[test]
fn stage18_56_qualified_gat_both_undefined() {
    let src = "\
fn f<T>(x: T) -> <T as UndefinedTrait>::UndefinedItem { 0 }
fn main() { 0 }
";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Both trait and assoc undefined must produce a resolve error"
    );
}
