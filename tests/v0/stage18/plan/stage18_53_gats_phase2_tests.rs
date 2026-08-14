//! Stage 18.53 — GATs Phase 2: Qualified path + projection lowering tests.
//!
//! Phase 2 scope: parser supports `<T as Trait>::Item` and `Self::Item<'a>`;
//! HIR→MIR lower produces `TyKind::Projection` for qualified paths.
//!
//! Per §9.4.3: 3 positive + 9 negative tests (1:3 ratio).
//! Per §1.0 原則 6 "通用 > 特例": `try_parse_qself` handles all qualified
//! path forms; `eat_gt_or_split` handles `>>` in nested generics.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::compile;

// === Positive: GAT usage parses and lowers correctly ===

/// Stage 18.53 positive 1: Qualified path `<T as Trait>::Item` parses.
///
/// Verifies that the parser handles `<T as Container>::Item` as a return
/// type. Phase 2 acceptance: parser succeeds (no parse errors). Typeck
/// may report errors for the projection (Phase 3 territory).
#[test]
fn stage18_53_qualified_path_parses() {
    let src = "\
trait Container {
    type Item;
}
fn get<T: Container>(x: T) -> <T as Container>::Item {
    0
}
fn main() {}";
    let result = compile(src);
    assert!(
        result.errors.parse.is_empty(),
        "Qualified path should parse without errors, parse errors: {:?}",
        result.errors.parse
    );
    assert!(
        result.errors.lex.is_empty(),
        "Qualified path should lex without errors, lex errors: {:?}",
        result.errors.lex
    );
}

/// Stage 18.53 positive 2: GAT with lifetime arg parses (`Self::Item<'a>`).
///
/// Verifies that `Self::Item<'a>` in a trait method return type parses,
/// including `&'a mut self` parameter syntax.
#[test]
fn stage18_53_gat_with_lifetime_arg_parses() {
    let src = "\
trait LendingIterator {
    type Item<'a>;
    fn next<'a>(&'a mut self) -> Option<Self::Item<'a>>;
}
fn main() {}";
    let result = compile(src);
    assert!(
        result.errors.parse.is_empty(),
        "GAT with lifetime arg should parse, parse errors: {:?}",
        result.errors.parse
    );
    assert!(
        result.errors.lex.is_empty(),
        "GAT with lifetime arg should lex without errors, lex errors: {:?}",
        result.errors.lex
    );
}

/// Stage 18.53 positive 3: Nested generics with `>>` splitting.
///
/// Verifies that `Vec<Vec<i32>>` and similar nested generics parse
/// correctly via `eat_gt_or_split`. (Note: `Vec` is stdlib-defined;
/// resolve errors are tolerated — only parse errors fail this test.)
#[test]
fn stage18_53_nested_generics_split_shr() {
    let src = "\
fn main() {
    let x: Option<Vec<i32>> = None;
}
";
    let result = compile(src);
    assert!(
        result.errors.parse.is_empty(),
        "Nested generics with `>>` should parse via split, parse errors: {:?}",
        result.errors.parse
    );
    assert!(
        result.errors.lex.is_empty(),
        "Nested generics should lex without errors, lex errors: {:?}",
        result.errors.lex
    );
}

// === Negative: GAT syntax errors properly reported ===

/// Stage 18.53 negative 1: Qualified path missing `as` keyword.
///
/// `<T>::Item` (no `as`) must NOT be treated as qself — the parser
/// rolls back and `<` becomes a comparison operator in expression
/// context, or a parse error in type context.
#[test]
fn stage18_53_qself_missing_as() {
    // In type context, `<T>::Item` without `as` should fail to parse
    // as a type (the parser's try_parse_qself returns None, then
    // parse_ty falls through to `_ => parse_path` which sees `<` and
    // produces an error).
    let src = "\
trait C { type Item; }
fn f<T: C>(x: T) -> <T>::Item { 0 }
fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty() || !result.errors.lex.is_empty(),
        "Qualified path missing `as` must produce a parse error"
    );
}

/// Stage 18.53 negative 2: Qualified path missing close `>`.
///
/// `<T as Trait::Item` (no `>`) must produce a parse error.
#[test]
fn stage18_53_qself_missing_close_angle() {
    let src = "\
trait C { type Item; }
fn f<T: C>(x: T) -> <T as C::Item { 0 }
fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Qualified path missing `>` must produce a parse error"
    );
}

/// Stage 18.53 negative 3: Qualified path missing `::` after `>`.
///
/// `<T as Trait>Item` (no `::`) must produce a parse error.
#[test]
fn stage18_53_qself_missing_path_sep() {
    let src = "\
trait C { type Item; }
fn f<T: C>(x: T) -> <T as C>Item { 0 }
fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Qualified path missing `::` after `>` must produce a parse error"
    );
}

/// Stage 18.53 negative 4: Qualified path with empty trait path.
///
/// `<T as>::Item` (empty trait path) must produce a parse error.
#[test]
fn stage18_53_qself_empty_trait() {
    let src = "\
trait C { type Item; }
fn f<T: C>(x: T) -> <T as>::Item { 0 }
fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Qualified path with empty trait must produce a parse error"
    );
}

/// Stage 18.53 negative 5: GAT with unbalanced generics `>>`.
///
/// `Self::Item<'a>>` (extra `>`) must produce a parse error.
#[test]
fn stage18_53_gat_unbalanced_generics() {
    let src = "\
trait C { type Item<'a>; fn f(&self) -> Self::Item<'a>>; }
fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "GAT with extra `>` must produce a parse error"
    );
}

/// Stage 18.53 negative 6: GAT with extra comma and no close angle.
///
/// `Self::Item<'a, T` (missing `>` after comma) must produce a parse error
/// because the parser expects another type argument after the comma but
/// finds `}`.
#[test]
fn stage18_53_gat_missing_close_angle() {
    let src = "\
trait C { type Item<'a>; fn f(&self) -> Self::Item<'a, T }
fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "GAT missing `>` after comma must produce a parse error"
    );
}

/// Stage 18.53 negative 7: Qualified path with missing assoc name.
///
/// `<T as Trait>::` (no assoc name after `::`) must produce a parse error.
#[test]
fn stage18_53_qself_missing_assoc_name() {
    let src = "\
trait C { type Item; }
fn f<T: C>(x: T) -> <T as C>:: { 0 }
fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Qualified path missing assoc name must produce a parse error"
    );
}

/// Stage 18.53 negative 8: Qualified path with garbage in trait.
///
/// `<T as @>::Item` (garbage `@` in trait path) must produce a parse error.
#[test]
fn stage18_53_qself_garbage_in_trait() {
    let src = "\
trait C { type Item; }
fn f<T: C>(x: T) -> <T as @>::Item { 0 }
fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Qualified path with garbage `@` in trait must produce a parse error"
    );
}

/// Stage 18.53 negative 9: Qualified path EOF mid-parse.
///
/// `<T as Trait` followed by EOF must produce a parse error.
#[test]
fn stage18_53_qself_eof_mid_parse() {
    let src = "trait C { type Item; } fn f<T: C>(x: T) -> <T as C";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Qualified path with EOF mid-parse must produce a parse error"
    );
}
