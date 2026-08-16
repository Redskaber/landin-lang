//! Stage 18.160 (TD-NEGATIVE-TEST-COVERAGE): Parser/Lexer negative tests.
//!
//! Tests parser and lexer error paths. Per §9.4.3, negative tests should
//! be ≥25% of total. This file covers parse/lex error paths.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`).

use landin_compiler::compile;

// === Lexer errors ===

/// Stage 18.160 negative 1: unterminated string literal.
#[test]
fn stage18_160_lex_unterminated_string() {
    let result = compile(r#"fn main() { let s = "unterminated; }"#);
    assert!(
        !result.errors.lex.is_empty() || !result.errors.parse.is_empty(),
        "unterminated string should produce errors"
    );
}

/// Stage 18.160 negative 2: invalid character.
#[test]
fn stage18_160_lex_invalid_char() {
    let result = compile("fn main() { let x = @; }");
    assert!(
        !result.errors.lex.is_empty() || !result.errors.parse.is_empty(),
        "invalid char @ should produce errors"
    );
}

/// Stage 18.160 negative 3: invalid number format doesn't panic.
#[test]
fn stage18_160_lex_invalid_number() {
    let result = compile("fn main() { let x = 12.34.56; }");
    // Per §2 原则 9: compiler should not panic on invalid number.
    // Parser may accept as float field access (12.34 . 56).
    assert!(
        !result.mirs.is_empty() || result.has_errors(),
        "should produce MIR or errors"
    );
}

// === Parser errors ===

/// Stage 18.160 negative 4: missing semicolon.
#[test]
fn stage18_160_parse_missing_semicolon() {
    let result = compile("fn main() { let x = 42 }");
    assert!(
        !result.errors.parse.is_empty(),
        "missing semicolon should produce parse errors"
    );
}

/// Stage 18.160 negative 5: missing closing brace.
#[test]
fn stage18_160_parse_missing_brace() {
    let result = compile("fn main() { let x = 42;");
    assert!(
        !result.errors.parse.is_empty(),
        "missing closing brace should produce parse errors"
    );
}

/// Stage 18.160 negative 6: missing closing paren.
#[test]
fn stage18_160_parse_missing_paren() {
    let result = compile("fn main( { }");
    assert!(
        !result.errors.parse.is_empty(),
        "missing closing paren should produce parse errors"
    );
}

/// Stage 18.160 negative 7: invalid function declaration.
#[test]
fn stage18_160_parse_invalid_fn_decl() {
    let result = compile("fn { }");
    assert!(
        !result.errors.parse.is_empty(),
        "invalid fn declaration should produce parse errors"
    );
}

/// Stage 18.160 negative 8: invalid struct declaration.
#[test]
fn stage18_160_parse_invalid_struct() {
    let result = compile("struct { x: i32 }");
    assert!(
        !result.errors.parse.is_empty(),
        "invalid struct declaration should produce parse errors"
    );
}

/// Stage 18.160 negative 9: missing function name.
#[test]
fn stage18_160_parse_missing_fn_name() {
    let result = compile("fn () -> i32 { 42 }");
    assert!(
        !result.errors.parse.is_empty(),
        "missing fn name should produce parse errors"
    );
}

/// Stage 18.160 negative 10: invalid expression doesn't panic.
#[test]
fn stage18_160_parse_invalid_expr() {
    let result = compile("fn main() { let x = + +; }");
    // Per §2 原则 9: compiler should not panic on invalid expression.
    assert!(
        !result.mirs.is_empty() || result.has_errors(),
        "should produce MIR or errors"
    );
}

/// Stage 18.160 negative 11: missing type annotation doesn't panic.
#[test]
fn stage18_160_parse_missing_type() {
    let result = compile("fn main() { let x: ; }");
    // Per §2 原则 9: compiler should not panic on missing type.
    assert!(
        !result.mirs.is_empty() || result.has_errors(),
        "should produce MIR or errors"
    );
}

/// Stage 18.160 negative 12: invalid match arm doesn't panic.
#[test]
fn stage18_160_parse_invalid_match_arm() {
    let result = compile("fn main() { match 1 { 1 => } }");
    // Per §2 原则 9: compiler should not panic on invalid match arm.
    assert!(
        !result.mirs.is_empty() || result.has_errors(),
        "should produce MIR or errors"
    );
}

/// Stage 18.160 negative 13: missing arrow in match.
#[test]
fn stage18_160_parse_missing_match_arrow() {
    let result = compile("fn main() { match 1 { 1 2 } }");
    assert!(
        !result.errors.parse.is_empty(),
        "missing match arrow should produce parse errors"
    );
}

/// Stage 18.160 negative 14: invalid if condition.
#[test]
fn stage18_160_parse_invalid_if() {
    let result = compile("fn main() { if { } }");
    assert!(
        !result.errors.parse.is_empty(),
        "invalid if condition should produce parse errors"
    );
}

/// Stage 18.160 negative 15: incomplete for loop.
#[test]
fn stage18_160_parse_incomplete_for() {
    let result = compile("fn main() { for { } }");
    assert!(
        !result.errors.parse.is_empty(),
        "incomplete for loop should produce parse errors"
    );
}

/// Stage 18.160 negative 16: invalid let pattern.
#[test]
fn stage18_160_parse_invalid_let() {
    let result = compile("fn main() { let = 42; }");
    assert!(
        !result.errors.parse.is_empty(),
        "invalid let pattern should produce parse errors"
    );
}

/// Stage 18.160 negative 17: function with invalid body (missing closing brace).
#[test]
fn stage18_160_parse_missing_fn_body() {
    let result = compile("fn main() {");
    // Per §2 原则 4: missing closing brace should be reported as parse error.
    assert!(
        result.has_errors(),
        "missing closing brace should produce errors, got: {:?}",
        result.errors
    );
}

/// Stage 18.160 negative 18: extra tokens after function.
#[test]
fn stage18_160_parse_extra_tokens() {
    let result = compile("fn main() { } extra tokens");
    assert!(
        !result.errors.parse.is_empty(),
        "extra tokens should produce parse errors"
    );
}

/// Stage 18.160 negative 19: invalid use declaration doesn't panic.
#[test]
fn stage18_160_parse_invalid_use() {
    let result = compile("use ; fn main() { }");
    // Per §2 原则 9: compiler should not panic on invalid use.
    assert!(
        !result.mirs.is_empty() || result.has_errors(),
        "should produce MIR or errors"
    );
}

/// Stage 18.160 negative 20: invalid mod declaration.
#[test]
fn stage18_160_parse_invalid_mod() {
    let result = compile("mod ; fn main() { }");
    assert!(
        !result.errors.parse.is_empty(),
        "invalid mod declaration should produce parse errors"
    );
}
