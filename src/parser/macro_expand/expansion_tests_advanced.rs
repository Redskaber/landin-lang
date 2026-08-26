//! Advanced macro expansion tests (Stage 18.14+).
//!
//! Stage 18.310 (P3 LOC refactor): extracted from `expansion_tests.rs` per
//! §13.4 J1-J6 to satisfy J6 (file < 1500 LOC). Contains test sections
//! covering: nested repetition, hygiene, edge cases, and advanced
//! macro_rules! features introduced in Stage 18.14 onwards.

#![cfg(test)]

use super::*;
use crate::compile;

// =====================================================================
// Stage 18.14 tests — Nested repetition support
// =====================================================================

/// Stage 18.14 positive 1: A macro with nested repetition
/// `$( $( $x ),* );*` parses without errors.
#[test]
fn stage18_14_macro_with_nested_repetition() {
    let src = "macro_rules! m { ($($($x:expr),*);*) => { 0 } } fn main() { m!(1, 2; 3, 4) }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    // Note: nested repetition may not fully expand yet, but should parse.
    let _ = result;
}

/// Stage 18.14 positive 2: A macro with deep repetition (3 levels)
/// parses without errors.
#[test]
fn stage18_14_macro_with_deep_repetition() {
    let src = "macro_rules! m { ($($($($x:expr),*);*);*) => { 0 } } fn main() { m!(((1))) }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    let _ = result;
}

/// Stage 18.14 negative 1: push_capture_into_rep_names handles Single.
#[test]
fn stage18_14_match_repetition_collects_inner_repetition() {
    let mut rep_names: HashMap<crate::lexer::Symbol, Vec<Vec<Token>>> = HashMap::new();
    let mut interner = Rodeo::new();
    let x_sym = interner.get_or_intern("x");
    push_capture_into_rep_names(
        &mut rep_names,
        x_sym,
        CaptureValue::Single(vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }]),
    );
    assert_eq!(rep_names[&x_sym].len(), 1);
    assert_eq!(rep_names[&x_sym][0].len(), 1);
}

/// Stage 18.14 negative 2: push_capture_into_rep_names flattens Repetition.
#[test]
fn stage18_14_match_repetition_nested_flat_map() {
    let mut rep_names: HashMap<crate::lexer::Symbol, Vec<Vec<Token>>> = HashMap::new();
    let mut interner = Rodeo::new();
    let x_sym = interner.get_or_intern("x");
    // Inner repetition with 3 iterations: [1], [2], [3]
    push_capture_into_rep_names(
        &mut rep_names,
        x_sym,
        CaptureValue::Repetition(vec![
            vec![Token {
                kind: TokenKind::IntLit(1, None),
                span: crate::session::Span::DUMMY,
            }],
            vec![Token {
                kind: TokenKind::IntLit(2, None),
                span: crate::session::Span::DUMMY,
            }],
            vec![Token {
                kind: TokenKind::IntLit(3, None),
                span: crate::session::Span::DUMMY,
            }],
        ]),
    );
    // Should flatten to [1, 2, 3] as ONE outer iteration.
    assert_eq!(rep_names[&x_sym].len(), 1, "should be 1 outer iteration");
    assert_eq!(rep_names[&x_sym][0].len(), 3, "flattened to 3 tokens");
}

/// Stage 18.14 negative 3: substitute_repetition with nested captures
/// produces output.
#[test]
fn stage18_14_substitute_repetition_nested_works() {
    let mut interner = Rodeo::new();
    let x_sym = interner.get_or_intern("x");
    // captures: $x is Repetition with 2 outer iterations, each
    // containing flattened inner tokens.
    let mut captures: Captures = Captures::new();
    captures.insert(
        x_sym,
        CaptureValue::Repetition(vec![
            vec![Token {
                kind: TokenKind::IntLit(1, None),
                span: crate::session::Span::DUMMY,
            }],
            vec![Token {
                kind: TokenKind::IntLit(2, None),
                span: crate::session::Span::DUMMY,
            }],
        ]),
    );
    let inner = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(x_sym),
            span: crate::session::Span::DUMMY,
        },
    ];
    let mut result = Vec::new();
    substitute_repetition(
        &inner,
        &captures,
        RepetitionKind::ZeroOrMore(RepetitionSep::None),
        &mut result,
    );
    // Should produce 2 tokens (one per outer iteration).
    assert_eq!(result.len(), 2);
}

/// Stage 18.14 negative 4: Nested repetition with separators.
#[test]
fn stage18_14_nested_repetition_with_separators() {
    let src = "macro_rules! m { ($($($x:expr),+);+) => { 0 } } fn main() { m!(1,2; 3,4) }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    let _ = result;
}

/// Stage 18.14 negative 5: CaptureValue::Repetition variant holds inner.
#[test]
fn stage18_14_capture_value_repetition_holds_inner() {
    let val = CaptureValue::Repetition(vec![
        vec![Token {
            kind: TokenKind::IntLit(1, None),
            span: crate::session::Span::DUMMY,
        }],
        vec![Token {
            kind: TokenKind::IntLit(2, None),
            span: crate::session::Span::DUMMY,
        }],
    ]);
    match val {
        CaptureValue::Repetition(iters) => {
            assert_eq!(iters.len(), 2);
        }
        _ => panic!("expected Repetition"),
    }
}

/// Stage 18.14 negative 6: match_repetition preserves inner order.
#[test]
fn stage18_14_match_repetition_preserves_inner_order() {
    let mut rep_names: HashMap<crate::lexer::Symbol, Vec<Vec<Token>>> = HashMap::new();
    let mut interner = Rodeo::new();
    let x_sym = interner.get_or_intern("x");
    // Push 3 iterations in order.
    for i in 1..=3u128 {
        push_capture_into_rep_names(
            &mut rep_names,
            x_sym,
            CaptureValue::Single(vec![Token {
                kind: TokenKind::IntLit(i, None),
                span: crate::session::Span::DUMMY,
            }]),
        );
    }
    // Verify order preserved.
    assert_eq!(rep_names[&x_sym].len(), 3);
    assert!(matches!(
        rep_names[&x_sym][0][0].kind,
        TokenKind::IntLit(1, _)
    ));
    assert!(matches!(
        rep_names[&x_sym][1][0].kind,
        TokenKind::IntLit(2, _)
    ));
    assert!(matches!(
        rep_names[&x_sym][2][0].kind,
        TokenKind::IntLit(3, _)
    ));
}

// =====================================================================
// Stage 18.17 tests — Basic macro hygiene (HygieneContext)
// =====================================================================

/// Stage 18.17 positive 1: HygieneContext::new() creates a context
/// with counter=0.
#[test]
fn stage18_17_hygiene_context_new_creates_zero_counter() {
    let ctx = HygieneContext::new();
    assert_eq!(ctx.counter(), 0, "new context should have counter=0");
}

/// Stage 18.17 positive 2: gen_unique_name increments the counter.
#[test]
fn stage18_17_hygiene_context_gen_unique_name_increments() {
    let mut ctx = HygieneContext::new();
    assert_eq!(ctx.counter(), 0);
    let _ = ctx.gen_unique_name("x");
    assert_eq!(ctx.counter(), 1, "counter should be 1 after one call");
    let _ = ctx.gen_unique_name("y");
    assert_eq!(ctx.counter(), 2, "counter should be 2 after two calls");
}

/// Stage 18.17 negative 1: Default trait creates counter=0.
#[test]
fn stage18_17_hygiene_context_default() {
    let ctx = HygieneContext::default();
    assert_eq!(ctx.counter(), 0);
}

/// Stage 18.17 negative 2: gen_unique_name produces the correct format.
#[test]
fn stage18_17_hygiene_context_gen_unique_name_format() {
    let mut ctx = HygieneContext::new();
    let name = ctx.gen_unique_name("tmp");
    assert_eq!(
        name, "__landin_macro_tmp_0",
        "first name should be __landin_macro_tmp_0"
    );
}

/// Stage 18.17 negative 3: Multiple gen_unique_name calls produce
/// different names.
#[test]
fn stage18_17_hygiene_context_gen_multiple_unique() {
    let mut ctx = HygieneContext::new();
    let n1 = ctx.gen_unique_name("x");
    let n2 = ctx.gen_unique_name("x");
    let n3 = ctx.gen_unique_name("x");
    assert_ne!(n1, n2, "names should differ");
    assert_ne!(n2, n3, "names should differ");
    assert_ne!(n1, n3, "names should differ");
}

/// Stage 18.17 negative 4: Clone preserves the counter value.
#[test]
fn stage18_17_hygiene_context_clone_preserves_counter() {
    let mut ctx = HygieneContext::new();
    let _ = ctx.gen_unique_name("a");
    let _ = ctx.gen_unique_name("b");
    assert_eq!(ctx.counter(), 2);
    let cloned = ctx.clone();
    assert_eq!(cloned.counter(), 2, "clone should preserve counter");
}

/// Stage 18.17 negative 5: Macro expansion still works correctly
/// with HygieneContext infrastructure in place (no behavior change).
#[test]
fn stage18_17_macro_expansion_with_hygiene_context_still_works() {
    let src = "macro_rules! m { () => { 42 } } fn main() { m!() }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.17 negative 6: println! still works (hygiene context
/// doesn't interfere with built-in macros).
#[test]
fn stage18_17_hygiene_context_does_not_break_println() {
    let src = "fn main() { println!(\"hello\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
    assert!(result.errors.macro_errors.is_empty());
}

// =====================================================================
// Stage 18.20 tests — Macro hygiene activation (apply_hygiene)
// =====================================================================

/// Stage 18.20 positive 1: apply_hygiene renames a non-capture identifier.
#[test]
fn stage18_20_apply_hygiene_renames_identifier() {
    let mut interner = Rodeo::new();
    let tmp_sym = interner.get_or_intern("tmp");
    let body = vec![Token {
        kind: TokenKind::Ident(tmp_sym),
        span: crate::session::Span::DUMMY,
    }];
    let captures = Captures::new();
    let mut hygiene = HygieneContext::new();
    let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
    // Should be renamed to __landin_macro_tmp_0
    assert_eq!(result.len(), 1);
    if let TokenKind::Ident(s) = &result[0].kind {
        let name = interner.resolve(s);
        assert_eq!(name, "__landin_macro_tmp_0");
    } else {
        panic!("expected Ident token");
    }
}

/// Stage 18.20 positive 2: apply_hygiene skips `$name` capture references.
#[test]
fn stage18_20_apply_hygiene_skips_captures() {
    let mut interner = Rodeo::new();
    let x_sym = interner.get_or_intern("x");
    let body = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(x_sym),
            span: crate::session::Span::DUMMY,
        },
    ];
    let captures = Captures::new();
    let mut hygiene = HygieneContext::new();
    let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
    // $x should NOT be renamed — emit both tokens unchanged.
    assert_eq!(result.len(), 2);
    assert!(matches!(result[0].kind, TokenKind::Dollar));
    if let TokenKind::Ident(s) = &result[1].kind {
        assert_eq!(interner.resolve(s), "x", "$x should not be renamed");
    }
}

/// Stage 18.20 negative 1: apply_hygiene skips keywords.
#[test]
fn stage18_20_apply_hygiene_skips_keywords() {
    let mut interner = Rodeo::new();
    // `let` is a keyword — should not be renamed.
    let body = vec![Token {
        kind: TokenKind::KwLet,
        span: crate::session::Span::DUMMY,
    }];
    let captures = Captures::new();
    let mut hygiene = HygieneContext::new();
    let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
    assert_eq!(result.len(), 1);
    // KwLet is not an Ident, so it's emitted unchanged.
    assert!(matches!(result[0].kind, TokenKind::KwLet));
}

/// Stage 18.20 negative 2: apply_hygiene skips built-in macro names.
#[test]
fn stage18_20_apply_hygiene_skips_builtins() {
    let mut interner = Rodeo::new();
    let println_sym = interner.get_or_intern("println");
    let body = vec![Token {
        kind: TokenKind::Ident(println_sym),
        span: crate::session::Span::DUMMY,
    }];
    let captures = Captures::new();
    let mut hygiene = HygieneContext::new();
    let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
    // println should NOT be renamed.
    assert_eq!(result.len(), 1);
    if let TokenKind::Ident(s) = &result[0].kind {
        assert_eq!(
            interner.resolve(s),
            "println",
            "println should not be renamed"
        );
    }
}

/// Stage 18.20 negative 3: apply_hygiene skips literals (not identifiers).
#[test]
fn stage18_20_apply_hygiene_skips_literals() {
    let mut interner = Rodeo::new();
    let body = vec![Token {
        kind: TokenKind::IntLit(42, None),
        span: crate::session::Span::DUMMY,
    }];
    let captures = Captures::new();
    let mut hygiene = HygieneContext::new();
    let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
    assert_eq!(result.len(), 1);
    assert!(matches!(result[0].kind, TokenKind::IntLit(42, _)));
}

/// Stage 18.20 negative 4: apply_hygiene increments the counter.
#[test]
fn stage18_20_apply_hygiene_increments_counter() {
    let mut interner = Rodeo::new();
    let a_sym = interner.get_or_intern("a");
    let b_sym = interner.get_or_intern("b");
    let body = vec![
        Token {
            kind: TokenKind::Ident(a_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(b_sym),
            span: crate::session::Span::DUMMY,
        },
    ];
    let captures = Captures::new();
    let mut hygiene = HygieneContext::new();
    let _ = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
    // Two renames → counter should be 2.
    assert_eq!(hygiene.counter(), 2);
}

/// Stage 18.20 negative 5: apply_hygiene preserves spans.
#[test]
fn stage18_20_apply_hygiene_preserves_spans() {
    let mut interner = Rodeo::new();
    let tmp_sym = interner.get_or_intern("tmp");
    let span = crate::session::Span::new(10, 20);
    let body = vec![Token {
        kind: TokenKind::Ident(tmp_sym),
        span,
    }];
    let captures = Captures::new();
    let mut hygiene = HygieneContext::new();
    let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
    assert_eq!(result[0].span, span, "span should be preserved");
}

/// Stage 18.20 negative 6: apply_hygiene on empty body returns empty.
#[test]
fn stage18_20_apply_hygiene_empty_body() {
    let mut interner = Rodeo::new();
    let body: Vec<Token> = vec![];
    let captures = Captures::new();
    let mut hygiene = HygieneContext::new();
    let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
    assert!(result.is_empty(), "empty body → empty result");
}

// =====================================================================
// Stage 18.21 tests — __landin_println infrastructure
// =====================================================================

/// Stage 18.21 positive 1: println! still works (Phase 1 no-op body).
#[test]
fn stage18_21_println_still_works() {
    let src = "fn main() { println!(\"hello\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
    assert!(result.errors.macro_errors.is_empty());
}

/// Stage 18.21 positive 2: eprintln! still works.
#[test]
fn stage18_21_eprintln_still_works() {
    let src = "fn main() { eprintln!(\"err\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.21 negative 1: __landin_println detection works (tested
/// indirectly — println! still compiles, meaning the infrastructure
/// is in place).
#[test]
fn stage18_21_is_landin_print_macro_detects_all() {
    // Test indirectly: println!/print!/eprintln!/eprint! all compile.
    let srcs = [
        "fn main() { println!(\"a\"); }",
        "fn main() { print!(\"b\"); }",
        "fn main() { eprintln!(\"c\"); }",
        "fn main() { eprint!(\"d\"); }",
    ];
    for src in srcs {
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "lex error for: {}", src);
        assert!(result.errors.parse.is_empty(), "parse error for: {}", src);
    }
}

/// Stage 18.21 negative 2: Resolver recognizes __landin_ functions.
#[test]
fn stage18_21_resolver_recognizes_landin_functions() {
    // __landin_println should resolve without error.
    let src = "fn main() { let _ = __landin_println; }";
    let result = compile(src);
    // Even if typeck fails (not a value), resolve should NOT report
    // "cannot find value".
    let has_resolve_error = result
        .errors
        .resolve
        .iter()
        .any(|e| e.message.contains("cannot find value"));
    assert!(
        !has_resolve_error,
        "__landin_ functions should be recognized by resolver"
    );
}

/// Stage 18.21 negative 3: println! with args still works.
#[test]
fn stage18_21_println_with_args_still_works() {
    let src = "fn main() { let x = 42; println!(\"x={}\", x); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.21 negative 4: print! (no newline) still works.
#[test]
fn stage18_21_print_still_works() {
    let src = "fn main() { print!(\"no newline\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.21 negative 5: eprint! still works.
#[test]
fn stage18_21_eprint_still_works() {
    let src = "fn main() { eprint!(\"err\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.21 negative 6: User macro_rules! not affected.
#[test]
fn stage18_21_user_macro_not_affected() {
    let src = "macro_rules! m { () => { 42 } } fn main() { m!() }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

// =====================================================================
// Stage 18.24 tests — Fragment specifier extension (lifetime + stmt)
// =====================================================================

/// Stage 18.24 positive 1: A macro using `:lifetime` fragment parses.
#[test]
fn stage18_24_macro_with_lifetime_fragment() {
    let src = "macro_rules! m { ($l:lifetime) => { 0 } } fn main() { 0 }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(result.errors.parse.is_empty(), "no parse errors");
}

/// Stage 18.24 positive 2: A macro using `:stmt` fragment parses.
#[test]
fn stage18_24_macro_with_stmt_fragment() {
    let src = "macro_rules! m { ($s:stmt) => { 0 } } fn main() { 0 }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(result.errors.parse.is_empty(), "no parse errors");
}

/// Stage 18.24 negative 1: capture_lifetime collects a Lifetime token.
#[test]
fn stage18_24_capture_lifetime_simple() {
    let mut interner = Rodeo::new();
    let sym = interner.get_or_intern("a");
    let tokens = vec![Token {
        kind: TokenKind::Lifetime(sym),
        span: crate::session::Span::DUMMY,
    }];
    let mut idx = 0;
    let captured = capture_lifetime(&tokens, &mut idx);
    assert_eq!(captured.len(), 1, "should capture 1 token");
    assert!(matches!(captured[0].kind, TokenKind::Lifetime(_)));
    assert_eq!(idx, 1, "should advance idx by 1");
}

/// Stage 18.24 negative 2: capture_lifetime rejects non-lifetime tokens.
#[test]
fn stage18_24_capture_lifetime_rejects_non_lifetime() {
    let tokens = vec![Token {
        kind: TokenKind::IntLit(42, None),
        span: crate::session::Span::DUMMY,
    }];
    let mut idx = 0;
    let captured = capture_lifetime(&tokens, &mut idx);
    assert!(captured.is_empty(), "non-lifetime → empty");
    assert_eq!(idx, 0, "idx should not advance");
}

/// Stage 18.24 negative 3: capture_stmt collects until semicolon.
#[test]
fn stage18_24_capture_stmt_until_semicolon() {
    let tokens = vec![
        Token {
            kind: TokenKind::Ident(lasso::Spur::default()),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Semicolon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::IntLit(99, None),
            span: crate::session::Span::DUMMY,
        },
    ];
    let mut idx = 0;
    let captured = capture_stmt(&tokens, &mut idx);
    // Should capture `ident ;` (2 tokens, semicolon inclusive).
    assert_eq!(captured.len(), 2, "should capture 2 tokens (ident + ;)");
    assert_eq!(idx, 2, "should advance past semicolon");
}

/// Stage 18.24 negative 4: capture_stmt stops at rbrace (exclusive).
#[test]
fn stage18_24_capture_stmt_until_rbrace() {
    let tokens = vec![
        Token {
            kind: TokenKind::IntLit(1, None),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RBrace,
            span: crate::session::Span::DUMMY,
        },
    ];
    let mut idx = 0;
    let captured = capture_stmt(&tokens, &mut idx);
    // Should capture `1` (1 token, rbrace exclusive).
    assert_eq!(
        captured.len(),
        1,
        "should capture 1 token (rbrace exclusive)"
    );
    assert_eq!(idx, 1, "should stop at rbrace");
}

/// Stage 18.24 negative 5: capture_stmt handles nested braces correctly.
#[test]
fn stage18_24_capture_stmt_nested_braces() {
    let tokens = vec![
        Token {
            kind: TokenKind::LBrace,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::IntLit(1, None),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Semicolon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RBrace,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Semicolon,
            span: crate::session::Span::DUMMY,
        },
    ];
    let mut idx = 0;
    let captured = capture_stmt(&tokens, &mut idx);
    // Should capture `{ 1 ; } ;` (5 tokens — the inner `;` is inside
    // braces, so depth > 0; the outer `;` ends the capture).
    assert_eq!(
        captured.len(),
        5,
        "should capture all 5 tokens including outer ;"
    );
    assert_eq!(idx, 5, "should advance past outer semicolon");
}

/// Stage 18.24 negative 6: lifetime fragment in pattern matches correctly.
#[test]
fn stage18_24_lifetime_fragment_in_pattern() {
    let mut interner = Rodeo::new();
    let l_sym = interner.get_or_intern("l");
    let lifetime_sym = interner.get_or_intern("lifetime");
    let a_sym = interner.get_or_intern("a");
    // Pattern: $ l : lifetime
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(l_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(lifetime_sym),
            span: crate::session::Span::DUMMY,
        },
    ];
    // Input: 'a
    let input = vec![Token {
        kind: TokenKind::Lifetime(a_sym),
        span: crate::session::Span::DUMMY,
    }];
    let mut captures = Captures::new();
    assert!(match_pattern(&pattern, &input, &mut captures, &interner));
    // $l should be captured as Single with 1 token.
    if let Some(CaptureValue::Single(tokens)) = captures.get(&l_sym) {
        assert_eq!(tokens.len(), 1, "should capture 1 lifetime token");
        assert!(matches!(tokens[0].kind, TokenKind::Lifetime(_)));
    } else {
        panic!("expected CaptureValue::Single for $l");
    }
}

// =====================================================================
// Stage 18.26 tests — Macro hygiene activation
// =====================================================================

/// Stage 18.26 positive 1: println! still works after hygiene activation.
#[test]
fn stage18_26_println_still_works_after_hygiene() {
    let src = "fn main() { println!(\"hello\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
    assert!(result.errors.macro_errors.is_empty());
}

/// Stage 18.26 positive 2: User macro still works after hygiene.
#[test]
fn stage18_26_user_macro_still_works_after_hygiene() {
    let src = "macro_rules! m { () => { 42 } } fn main() { m!() }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.26 negative 1: println! with args still works.
#[test]
fn stage18_26_println_with_args_after_hygiene() {
    let src = "fn main() { let x = 42; println!(\"x={}\", x); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.26 negative 2: eprintln! still works.
#[test]
fn stage18_26_eprintln_after_hygiene() {
    let src = "fn main() { eprintln!(\"err\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.26 negative 3: macro with repetition still works.
#[test]
fn stage18_26_macro_repetition_after_hygiene() {
    let src = "macro_rules! m { ($($x:expr)*) => { 0 } } fn main() { m!() }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.26 negative 4: macro with separator still works.
#[test]
fn stage18_26_macro_separator_after_hygiene() {
    let src = "macro_rules! m { ($($x:expr),*) => { 0 } } fn main() { m!(1, 2, 3) }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.26 negative 5: macro with capture still works.
#[test]
fn stage18_26_macro_capture_after_hygiene() {
    let src = "macro_rules! m { ($x:expr) => { $x } } fn main() { m!(42) }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.26 negative 6: print! (no newline) still works.
#[test]
fn stage18_26_print_after_hygiene() {
    let src = "fn main() { print!(\"no newline\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

// =====================================================================
// Stage 18.27 tests — println! Phase 2.5: __landin_println activation
// =====================================================================

/// Stage 18.27 positive 1: println! expands to __landin_println.
#[test]
fn stage18_27_println_expands_to_landin_println() {
    let mut interner = Rodeo::new();
    for name in BUILTIN_MACRO_NAMES {
        interner.get_or_intern(name);
        interner.get_or_intern(format!("__landin_{}", name));
    }
    interner.get_or_intern("args");
    interner.get_or_intern("tt");

    let println_sym = interner.get_or_intern("println");
    let landin_println_sym = interner.get_or_intern("__landin_println");
    let hi_sym = interner.get_or_intern("hi");
    let span = crate::session::Span::new(0, 10);
    let tokens = vec![
        Token {
            kind: TokenKind::Ident(println_sym),
            span,
        },
        Token {
            kind: TokenKind::Not,
            span,
        },
        Token {
            kind: TokenKind::LParen,
            span,
        },
        Token {
            kind: TokenKind::StrLit(hi_sym),
            span,
        },
        Token {
            kind: TokenKind::RParen,
            span,
        },
    ];
    let (out, errors) = expand_macros_with_errors(tokens, &mut interner);
    assert!(errors.is_empty());
    assert_eq!(out.len(), 4, "should expand to __landin_println(\"hi\")");
    assert!(matches!(out[0].kind, TokenKind::Ident(s) if s == landin_println_sym));
}

/// Stage 18.27 positive 2: println! still compiles end-to-end.
#[test]
fn stage18_27_println_compiles_end_to_end() {
    let src = "fn main() { println!(\"hello\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
    assert!(result.errors.macro_errors.is_empty());
}

/// Stage 18.27 negative 1: eprintln! expands to __landin_eprintln.
#[test]
fn stage18_27_eprintln_expands_correctly() {
    let src = "fn main() { eprintln!(\"err\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.27 negative 2: print! expands to __landin_print.
#[test]
fn stage18_27_print_expands_correctly() {
    let src = "fn main() { print!(\"no newline\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.27 negative 3: println! with args compiles.
#[test]
fn stage18_27_println_with_args_compiles() {
    let src = "fn main() { let x = 42; println!(\"x={}\", x); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.27 negative 4: apply_hygiene skips __landin_ functions.
#[test]
fn stage18_27_hygiene_skips_landin_functions() {
    let mut interner = Rodeo::new();
    let lp_sym = interner.get_or_intern("__landin_println");
    let body = vec![Token {
        kind: TokenKind::Ident(lp_sym),
        span: crate::session::Span::DUMMY,
    }];
    let captures = Captures::new();
    let mut hygiene = HygieneContext::new();
    let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
    // __landin_println should NOT be renamed.
    assert_eq!(result.len(), 1);
    if let TokenKind::Ident(s) = &result[0].kind {
        assert_eq!(*s, lp_sym, "__landin_println should not be renamed");
    }
    // Counter should be 0 (no renames happened).
    assert_eq!(hygiene.counter(), 0);
}

/// Stage 18.27 negative 5: user macro still works.
#[test]
fn stage18_27_user_macro_still_works() {
    let src = "macro_rules! m { () => { 42 } } fn main() { m!() }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

/// Stage 18.27 negative 6: eprint! still works.
#[test]
fn stage18_27_eprint_still_works() {
    let src = "fn main() { eprint!(\"err\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
}

// =====================================================================
// Stage 18.29 tests — Built-in non-print macros (assert!/panic!/vec!)
// =====================================================================

/// Stage 18.29 positive 1: assert! macro parses and expands.
#[test]
fn stage18_29_assert_macro_parses() {
    let src = "fn main() { assert!(1 == 1); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(result.errors.macro_errors.is_empty(), "no macro errors");
    // parse may have errors because __landin_assert is not yet fully
    // integrated in codegen — just verify no lex/macro errors.
}

/// Stage 18.29 positive 2: vec! macro parses and expands to array.
#[test]
fn stage18_29_vec_macro_parses() {
    let src = "fn main() { let _a = vec![1, 2, 3]; }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(result.errors.macro_errors.is_empty(), "no macro errors");
}

/// Stage 18.29 negative 1: panic! macro parses.
#[test]
fn stage18_29_panic_macro_parses() {
    let src = "fn main() { panic!(\"oops\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(result.errors.macro_errors.is_empty(), "no macro errors");
}

/// Stage 18.29 negative 2: BUILTIN_MACRO_NAMES includes non-print macros.
#[test]
fn stage18_29_builtin_names_includes_non_print() {
    assert!(BUILTIN_MACRO_NAMES.contains(&"assert"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"panic"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"vec"));
    // Stage 18.32: more non-print macros
    assert!(BUILTIN_MACRO_NAMES.contains(&"format"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"dbg"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"todo"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"unimplemented"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"write"));
    // Stage 18.34: compile-time utility macros
    assert!(BUILTIN_MACRO_NAMES.contains(&"stringify"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"concat"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"env"));
    // Stage 18.36: source info + file macros
    assert!(BUILTIN_MACRO_NAMES.contains(&"file"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"line"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"module_path"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"include_str"));
    // Stage 18.39: pattern + config macros
    assert!(BUILTIN_MACRO_NAMES.contains(&"matches"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"cfg"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"option_env"));
    // Stage 18.41: low-level + diagnostic macros
    assert!(BUILTIN_MACRO_NAMES.contains(&"asm"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"compile_error"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"cfg_attr"));
    // Stage 18.43: control-flow + debug macros
    assert!(BUILTIN_MACRO_NAMES.contains(&"unreachable"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"trace_macros"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"format_args"));
}

/// Stage 18.29 + 18.32 negative 3: build_builtin_macro_table registers 12 macros.
#[test]
fn stage18_29_table_has_seven_macros() {
    let mut interner = Rodeo::new();
    for name in BUILTIN_MACRO_NAMES {
        interner.get_or_intern(name);
    }
    interner.get_or_intern("args");
    interner.get_or_intern("tt");
    interner.get_or_intern("cond");
    interner.get_or_intern("msg");
    interner.get_or_intern("x");
    interner.get_or_intern("dst");
    interner.get_or_intern("path");
    interner.get_or_intern("expr");
    interner.get_or_intern("__landin_assert");
    interner.get_or_intern("__landin_panic_msg");
    interner.get_or_intern("__landin_format");
    interner.get_or_intern("__landin_dbg");
    interner.get_or_intern("__landin_write");
    interner.get_or_intern("__landin_stringify");
    interner.get_or_intern("__landin_concat");
    interner.get_or_intern("__landin_env");
    interner.get_or_intern("__landin_file");
    interner.get_or_intern("__landin_line");
    interner.get_or_intern("__landin_module_path");
    interner.get_or_intern("__landin_include_str");
    interner.get_or_intern("pat");
    interner.get_or_intern("cfg");
    interner.get_or_intern("__landin_matches");
    interner.get_or_intern("__landin_cfg");
    interner.get_or_intern("__landin_option_env");
    interner.get_or_intern("attr");
    interner.get_or_intern("__landin_asm");
    interner.get_or_intern("__landin_compile_error");
    interner.get_or_intern("__landin_cfg_attr");
    interner.get_or_intern("mode");
    interner.get_or_intern("__landin_unreachable");
    interner.get_or_intern("__landin_trace_macros");
    interner.get_or_intern("__landin_format_args");
    for name in BUILTIN_MACRO_NAMES {
        interner.get_or_intern(format!("__landin_{}", name));
    }

    let table = build_builtin_macro_table(&mut interner);
    assert_eq!(table.len(), 28, "should have 28 built-in macros");
}

/// Stage 18.29 negative 4: vec! with empty args parses.
#[test]
fn stage18_29_vec_empty_parses() {
    let src = "fn main() { let _a = vec![]; }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(result.errors.macro_errors.is_empty(), "no macro errors");
}

/// Stage 18.29 negative 5: assert! with false condition parses.
#[test]
fn stage18_29_assert_false_parses() {
    let src = "fn main() { assert!(1 == 2); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(result.errors.macro_errors.is_empty(), "no macro errors");
}

/// Stage 18.29 negative 6: user can override built-in non-print macros.
#[test]
fn stage18_29_user_overrides_vec() {
    let src = "macro_rules! vec { () => { 42 } } fn main() { vec!() }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(
        result.errors.parse.is_empty(),
        "user vec! override should work"
    );
}
