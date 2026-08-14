//! Stage 18.52 — GATs (Generic Associated Types) Phase 1 tests.
//!
//! Phase 1 scope: AST / Parser / HIR infrastructure only.
//! These tests verify that GAT syntax parses correctly and the compiler
//! does not crash. Full type-checking of GAT projections is Phase 2+.
//!
//! Per §9.4.3: 2 positive + 6 negative tests (1:3 ratio).
//! Per §1.0 原則 6 "通用 > 特例": GAT parsing reuses existing
//! `parse_generics` / `parse_where_clause` infrastructure.
//!
//! Test design rationale:
//! - Positive tests verify that GAT syntax parses without lex/parse errors.
//!   Type-checking errors are tolerated (Phase 2 territory).
//! - Negative tests verify that genuinely invalid GAT syntax produces
//!   parse errors. Cases that the current Stage 0 parser silently accepts
//!   (e.g., `>>` treated as two `>` closes, missing `>` via `eat()`)
//!   are NOT used as negative tests — they are documented parser
//!   limitations, not GAT-specific bugs.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::compile;

// === Positive: GAT declarations parse correctly ===

/// Stage 18.52 positive 1: Simple GAT declaration with lifetime and default.
///
/// Verifies that `trait Foo { type Item<'a> = &'a i32; }` parses without
/// lex/parse errors. Phase 1 acceptance: parser succeeds. Full type-checking
/// of the GAT projection is Phase 2 territory.
#[test]
fn stage18_52_gat_parse_simple_lifetime_with_default() {
    let src = "\
trait LendingIterator {
    type Item<'a> = &'a i32;
}

fn main() {}";
    let result = compile(src);
    assert!(
        result.errors.parse.is_empty(),
        "GAT declaration should parse without errors, parse errors: {:?}",
        result.errors.parse
    );
    assert!(
        result.errors.lex.is_empty(),
        "GAT declaration should lex without errors, lex errors: {:?}",
        result.errors.lex
    );
    // HIR should be produced (parser + HIR lowering succeeded).
    assert!(
        result.hir.is_some(),
        "HIR should be produced for valid GAT declaration"
    );
}

/// Stage 18.52 positive 2: GAT with type parameter + where clause + impl.
///
/// Verifies that:
/// 1. `type Item<T> where T: Clone;` parses in trait declaration
/// 2. `type Item<i32> = i32;` parses in impl block
/// 3. Both lex/parse stages succeed
#[test]
fn stage18_52_gat_parse_with_type_param_where_and_impl() {
    let src = "\
trait Container {
    type Item<T> where T: Clone;
}

struct MyContainer;
impl Container for MyContainer {
    type Item<T> = i32;
}

fn main() {}";
    let result = compile(src);
    assert!(
        result.errors.parse.is_empty(),
        "GAT with type param + where + impl should parse, parse errors: {:?}",
        result.errors.parse
    );
    assert!(
        result.errors.lex.is_empty(),
        "GAT with type param should lex without errors, lex errors: {:?}",
        result.errors.lex
    );
    assert!(
        result.hir.is_some(),
        "HIR should be produced for valid GAT impl"
    );
}

// === Negative: GAT syntax errors properly reported ===

/// Stage 18.52 negative 1: Missing semicolon after GAT declaration.
///
/// `type Item<'a>` followed by `fn` (no `;`) must produce a parse error.
#[test]
fn stage18_52_gat_missing_semicolon() {
    let src = "\
trait Foo {
    type Item<'a>
    fn next(&self) -> i32;
}

fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Missing semicolon after GAT must produce a parse error, got: {:?}",
        result.errors.parse
    );
}

/// Stage 18.52 negative 2: Missing associated type name.
///
/// `type <'a>;` must produce a parse error (expected identifier).
#[test]
fn stage18_52_gat_missing_ident() {
    let src = "\
trait Foo {
    type <'a>;
}

fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Missing GAT identifier must produce a parse error, got: {:?}",
        result.errors.parse
    );
}

/// Stage 18.52 negative 3: EOF before semicolon.
///
/// `trait Foo { type Item<T>` followed by EOF must produce a parse error.
#[test]
fn stage18_52_gat_eof_no_semicolon() {
    let src = "trait Foo { type Item<T>";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "GAT declaration without `;` and EOF must produce a parse error, got: {:?}",
        result.errors.parse
    );
}

/// Stage 18.52 negative 4: Garbage character in GAT generics.
///
/// `type Item<@>;` must produce a parse error (`@` is not a valid generic param).
#[test]
fn stage18_52_gat_garbage_in_generics() {
    let src = "\
trait Foo {
    type Item<@>;
}

fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Garbage `@` in GAT generics must produce a parse error, got: {:?}",
        result.errors.parse
    );
}

/// Stage 18.52 negative 5: Numeric literal as generic parameter.
///
/// `type Item<42>;` must produce a parse error (numbers are not valid params).
#[test]
fn stage18_52_gat_number_as_param() {
    let src = "\
trait Foo {
    type Item<42>;
}

fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Numeric literal `42` as GAT param must produce a parse error, got: {:?}",
        result.errors.parse
    );
}

/// Stage 18.52 negative 6: Missing type name entirely.
///
/// `type ;` (no identifier at all) must produce a parse error.
#[test]
fn stage18_52_gat_missing_name_entirely() {
    let src = "\
trait Foo {
    type ;
}

fn main() {}";
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "Missing GAT name entirely must produce a parse error, got: {:?}",
        result.errors.parse
    );
}
