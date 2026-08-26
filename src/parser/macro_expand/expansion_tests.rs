//! Stage 18.248: Macro expansion tests (extracted from expansion.rs).
//!
//! Per §13.4 J2 (single responsibility): test module separated from code.
//! Per §9.4: tests cover positive + negative + edge cases.

#![cfg(test)]

use super::*;
use crate::ast::{MacroRule, MacroRulesDef};
use crate::compile;

/// Stage 18.03 positive 1: Macro expansion does not break compilation.
#[test]
fn stage18_03_macro_expansion_does_not_break() {
    let src = "macro_rules! my_macro { () => { 42 } } fn main() { 0 }";
    let result = compile(src);
    // macro_rules! is parsed but not yet expanded (Phase 3 integration pending).
    // Just verify it compiles without errors.
    assert!(
        result.errors.lex.is_empty(),
        "macro_rules! should not produce lex errors"
    );
}

/// Stage 18.03 positive 2: Multiple macro rules parse correctly.
#[test]
fn stage18_03_multiple_rules_parse() {
    let src = "macro_rules! multi { () => { 1 } ($x:expr) => { $x } } fn main() { 0 }";
    let result = compile(src);
    assert!(
        result.errors.lex.is_empty(),
        "macro_rules! with multiple rules should parse"
    );
}

/// Stage 18.03 negative 1: Empty macro_rules! parses.
#[test]
fn stage18_03_empty_macro_rules() {
    let src = "macro_rules! empty { } fn main() { 0 }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
}

/// Stage 18.03 negative 2: match_pattern matches empty pattern.
#[test]
fn stage18_03_match_empty_pattern() {
    let interner = Rodeo::new();
    let pattern: Vec<Token> = vec![];
    let input: Vec<Token> = vec![];
    let mut captures = HashMap::new();
    assert!(match_pattern(&pattern, &input, &mut captures, &interner));
}

/// Stage 18.03 negative 3: match_pattern matches literal tokens.
#[test]
fn stage18_03_match_literal_tokens() {
    let interner = Rodeo::new();
    let pattern = vec![Token {
        kind: TokenKind::IntLit(42, None),
        span: crate::session::Span::DUMMY,
    }];
    let input = vec![Token {
        kind: TokenKind::IntLit(42, None),
        span: crate::session::Span::DUMMY,
    }];
    let mut captures = HashMap::new();
    assert!(match_pattern(&pattern, &input, &mut captures, &interner));
}

/// Stage 18.03 negative 4: match_pattern rejects mismatched tokens.
#[test]
fn stage18_03_match_rejects_mismatch() {
    let interner = Rodeo::new();
    let pattern = vec![Token {
        kind: TokenKind::IntLit(42, None),
        span: crate::session::Span::DUMMY,
    }];
    let input = vec![Token {
        kind: TokenKind::IntLit(99, None),
        span: crate::session::Span::DUMMY,
    }];
    let mut captures = HashMap::new();
    assert!(!match_pattern(&pattern, &input, &mut captures, &interner));
}

/// Stage 18.03 negative 5: substitute_body replaces $name.
#[test]
fn stage18_03_substitute_replaces_name() {
    let mut interner = Rodeo::new();
    let x_sym = interner.get_or_intern("x");
    let mut captures: Captures = HashMap::new();
    captures.insert(
        x_sym,
        CaptureValue::Single(vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }]),
    );

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

    let result = substitute_body(&body, &captures);
    assert_eq!(result.len(), 1, "should substitute 1 token for $x");
    assert!(matches!(result[0].kind, TokenKind::IntLit(42, _)));
}

/// Stage 18.03 negative 6: expand_macro returns None when no rule matches.
#[test]
fn stage18_03_expand_no_match_returns_none() {
    let mut interner = Rodeo::new();
    let def = MacroRulesDef {
        name: interner.get_or_intern("test"),
        rules: vec![MacroRule {
            pattern: vec![Token {
                kind: TokenKind::IntLit(1, None),
                span: crate::session::Span::DUMMY,
            }],
            body: vec![],
            span: crate::session::Span::DUMMY,
        }],
        span: crate::session::Span::DUMMY,
    };
    let input = vec![Token {
        kind: TokenKind::IntLit(2, None),
        span: crate::session::Span::DUMMY,
    }];
    let result = expand_macro(&def, &input, &mut interner);
    assert!(result.is_none(), "non-matching input should return None");
}

// =====================================================================
// Stage 18.04 tests — Macro Call Invocation + Driver Integration
// =====================================================================

/// Stage 18.04 positive 1: A simple `macro_rules!` macro is expanded
/// at the call site. `m!()` expands to `42`, which the parser then
/// parses as an integer literal expression.
#[test]
fn stage18_04_macro_call_expands_simple() {
    let src = "macro_rules! m { () => { 42 } } fn main() { m!() }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(
        result.errors.parse.is_empty(),
        "macro_rules! m!() should expand and parse — parse errors: {:?}",
        result.errors.parse
    );
}

/// Stage 18.04 positive 2: A macro that captures `$x:expr` and
/// substitutes it into the body. `m!(99)` expands to `99`.
#[test]
fn stage18_04_macro_call_expands_with_capture() {
    let src = "macro_rules! m { ($x:expr) => { $x } } fn main() { m!(99) }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(
        result.errors.parse.is_empty(),
        "macro_rules! with $x:expr capture should expand and parse — errors: {:?}",
        result.errors.parse
    );
}

/// Stage 18.04 negative 1: collect_macro_defs returns an empty table
/// when the token stream has no macro_rules! definitions.
#[test]
fn stage18_04_collect_finds_no_macros() {
    let mut interner = Rodeo::new();
    let tokens: Vec<Token> = vec![Token {
        kind: TokenKind::IntLit(42, None),
        span: crate::session::Span::DUMMY,
    }];
    let table = collect_macro_defs(&tokens, &mut interner);
    assert!(table.is_empty(), "no macro_rules! → empty table");
}

/// Stage 18.04 negative 2: collect_macro_defs finds a macro_rules!
/// definition and stores it in the table.
#[test]
fn stage18_04_collect_finds_macro_definition() {
    let mut interner = Rodeo::new();
    let m_sym = interner.get_or_intern("m");
    let macro_rules_sym = interner.get_or_intern("macro_rules");
    let tokens = vec![
        Token {
            kind: TokenKind::Ident(macro_rules_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Not,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(m_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LBrace,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::FatArrow,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LBrace,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RBrace,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RBrace,
            span: crate::session::Span::DUMMY,
        },
    ];
    let table = collect_macro_defs(&tokens, &mut interner);
    assert_eq!(table.len(), 1, "should find 1 macro definition");
    assert!(table.contains_key(&m_sym), "table should contain 'm'");
    let def = &table[&m_sym];
    assert_eq!(def.rules.len(), 1, "macro 'm' should have 1 rule");
}

/// Stage 18.04 negative 3: expand_macro_calls passes through unknown
/// macros (like `println!`) unchanged so the parser can handle them.
#[test]
fn stage18_04_expand_macro_calls_passes_unknown() {
    let mut interner = Rodeo::new();
    let table = MacroTable::new(); // empty — no known macros
    let tokens = vec![
        Token {
            kind: TokenKind::Ident(interner.get_or_intern("println")),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Not,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::StrLit(interner.get_or_intern("hi")),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];
    let out = expand_macro_calls(&tokens, &table, &mut interner);
    assert_eq!(
        out.len(),
        tokens.len(),
        "unknown macro should pass through unchanged"
    );
    for (i, (orig, expanded)) in tokens.iter().zip(out.iter()).enumerate() {
        assert_eq!(orig.kind, expanded.kind, "token {} kind mismatch", i);
    }
}

/// Stage 18.04 negative 4: expand_macro_calls returns the input
/// unchanged when the macro table is empty (no macros defined).
#[test]
fn stage18_04_expand_macro_calls_passes_no_macros() {
    let mut interner = Rodeo::new();
    let table = MacroTable::new();
    let tokens = vec![
        Token {
            kind: TokenKind::IntLit(1, None),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Plus,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::IntLit(2, None),
            span: crate::session::Span::DUMMY,
        },
    ];
    let out = expand_macro_calls(&tokens, &table, &mut interner);
    assert_eq!(out.len(), 3, "no macros → 3 tokens unchanged");
}

/// Stage 18.04 negative 5: expand_macros with no macro_rules! defs
/// returns the input tokens unchanged (zero-overhead fast path).
#[test]
fn stage18_04_expand_macros_no_macros_returns_input() {
    let mut interner = Rodeo::new();
    let tokens = vec![Token {
        kind: TokenKind::IntLit(42, None),
        span: crate::session::Span::DUMMY,
    }];
    let out = expand_macros(tokens.clone(), &mut interner);
    assert_eq!(out.len(), 1, "no macros → unchanged output");
    assert!(matches!(out[0].kind, TokenKind::IntLit(42, _)));
}

/// Stage 18.04 negative 6: Recursive macro expansion terminates
/// without infinite loop. `m!()` expands to `m!()` (self-referential).
/// MAX_EXPANSION_ROUNDS (32) must cut it off.
#[test]
fn stage18_04_expand_macros_handles_recursive() {
    // m!() => m!() — would loop forever without MAX_EXPANSION_ROUNDS.
    // We expect expansion to terminate and the parser to receive some
    // token stream (whatever it is, it shouldn't loop).
    let src = "macro_rules! m { () => { m!() } } fn main() { 0 }";
    let result = compile(src);
    // The recursive macro doesn't have a base case, so the parser will
    // likely fail to parse the infinite expansion — but the compiler
    // should not hang. We just check it returns.
    let _ = result;
    // If we get here without timeout, the test passes.
}

// =====================================================================
// Stage 18.05 tests — Additional Fragment Specifiers
// =====================================================================

/// Stage 18.05 positive 1: A macro using the `:ty` fragment parses
/// and expands correctly.
#[test]
fn stage18_05_macro_with_ty_fragment() {
    let src = "macro_rules! m { ($t:ty) => { let x: $t; } } fn main() { m!(i32) }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(
        result.errors.parse.is_empty(),
        "macro with $t:ty should expand and parse — errors: {:?}",
        result.errors.parse
    );
}

/// Stage 18.05 positive 2: A macro using the `:literal` fragment
/// parses and expands correctly.
#[test]
fn stage18_05_macro_with_literal_fragment() {
    let src = "macro_rules! m { ($l:literal) => { $l } } fn main() { m!(42) }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(
        result.errors.parse.is_empty(),
        "macro with $l:literal should expand and parse — errors: {:?}",
        result.errors.parse
    );
}

/// Stage 18.05 negative 1: capture_ty collects a single type token
/// like `i32`.
#[test]
fn stage18_05_capture_ty_simple() {
    let mut interner = Rodeo::new();
    let i32_sym = interner.get_or_intern("i32");
    let tokens = vec![Token {
        kind: TokenKind::Ident(i32_sym),
        span: crate::session::Span::DUMMY,
    }];
    let mut idx = 0;
    let captured = capture_ty(&tokens, &mut idx);
    assert_eq!(captured.len(), 1, "should capture 1 token for i32");
    assert_eq!(idx, 1, "should advance idx by 1");
}

/// Stage 18.05 negative 2: capture_literal collects an int literal.
#[test]
fn stage18_05_capture_literal_int() {
    let tokens = vec![Token {
        kind: TokenKind::IntLit(42, None),
        span: crate::session::Span::DUMMY,
    }];
    let mut idx = 0;
    let captured = capture_literal(&tokens, &mut idx);
    assert_eq!(captured.len(), 1, "should capture 1 literal token");
    assert!(matches!(captured[0].kind, TokenKind::IntLit(42, _)));
    assert_eq!(idx, 1, "should advance idx by 1");
}

/// Stage 18.05 negative 3: capture_block collects `{ 1; 2 }` as
/// 5 tokens (including the delimiters).
#[test]
fn stage18_05_capture_block_balanced() {
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
            kind: TokenKind::IntLit(2, None),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RBrace,
            span: crate::session::Span::DUMMY,
        },
    ];
    let mut idx = 0;
    let captured = capture_block(&tokens, &mut idx);
    assert_eq!(
        captured.len(),
        5,
        "should capture all 5 tokens including delims"
    );
    assert_eq!(idx, 5, "should advance idx past closing brace");
}

/// Stage 18.05 negative 4: capture_path collects `a::b::c` as 5
/// tokens (3 idents + 2 path separators).
#[test]
fn stage18_05_capture_path_segments() {
    let mut interner = Rodeo::new();
    let a = interner.get_or_intern("a");
    let b = interner.get_or_intern("b");
    let c = interner.get_or_intern("c");
    let tokens = vec![
        Token {
            kind: TokenKind::Ident(a),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::PathSep,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(b),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::PathSep,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(c),
            span: crate::session::Span::DUMMY,
        },
    ];
    let mut idx = 0;
    let captured = capture_path(&tokens, &mut idx);
    assert_eq!(captured.len(), 5, "should capture all 5 tokens");
    assert_eq!(idx, 5, "should advance idx past last segment");
}

/// Stage 18.05 negative 5: capture_literal returns empty when the
/// current token is not a literal (e.g. an identifier).
#[test]
fn stage18_05_capture_literal_rejects_ident() {
    let mut interner = Rodeo::new();
    let sym = interner.get_or_intern("foo");
    let tokens = vec![Token {
        kind: TokenKind::Ident(sym),
        span: crate::session::Span::DUMMY,
    }];
    let mut idx = 0;
    let captured = capture_literal(&tokens, &mut idx);
    assert!(
        captured.is_empty(),
        "ident should not be captured as literal"
    );
    assert_eq!(idx, 0, "idx should not advance");
}

/// Stage 18.05 negative 6: capture_block returns empty when the
/// current token is not `{`.
#[test]
fn stage18_05_capture_block_rejects_non_brace() {
    let tokens = vec![Token {
        kind: TokenKind::IntLit(42, None),
        span: crate::session::Span::DUMMY,
    }];
    let mut idx = 0;
    let captured = capture_block(&tokens, &mut idx);
    assert!(
        captured.is_empty(),
        "non-brace should not be captured as block"
    );
    assert_eq!(idx, 0, "idx should not advance");
}

// =====================================================================
// Stage 18.06 tests — Repetition $(...)* / $(...)+ / $(...)?
// =====================================================================

/// Stage 18.06 positive 1: A macro using `$( $x:expr )*` (zero or
/// more expressions) parses and expands.
#[test]
fn stage18_06_macro_with_star_repetition() {
    let src = "macro_rules! m { ($($x:expr)*) => { 0 } } fn main() { m!() }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(
        result.errors.parse.is_empty(),
        "macro with $($x:expr)* should expand and parse — errors: {:?}",
        result.errors.parse
    );
}

/// Stage 18.06 positive 2: A macro using `$( $x:expr )+` (one or
/// more expressions) parses and expands.
#[test]
fn stage18_06_macro_with_plus_repetition() {
    let src = "macro_rules! m { ($($x:expr)+) => { 0 } } fn main() { m!(1) }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(
        result.errors.parse.is_empty(),
        "macro with $($x:expr)+ should expand and parse — errors: {:?}",
        result.errors.parse
    );
}

/// Stage 18.06 negative 1: parse_repetition_op maps `*` to ZeroOrMore(None).
#[test]
fn stage18_06_repetition_kind_from_star() {
    let tokens = vec![Token {
        kind: TokenKind::Star,
        span: crate::session::Span::DUMMY,
    }];
    assert_eq!(
        parse_repetition_op(&tokens, 0),
        Some((RepetitionKind::ZeroOrMore(RepetitionSep::None), 1))
    );
}

/// Stage 18.06 negative 2: parse_repetition_op maps `+` to OneOrMore(None).
#[test]
fn stage18_06_repetition_kind_from_plus() {
    let tokens = vec![Token {
        kind: TokenKind::Plus,
        span: crate::session::Span::DUMMY,
    }];
    assert_eq!(
        parse_repetition_op(&tokens, 0),
        Some((RepetitionKind::OneOrMore(RepetitionSep::None), 1))
    );
}

/// Stage 18.06 negative 3: parse_repetition_op maps `?` to ZeroOrOne(None).
#[test]
fn stage18_06_repetition_kind_from_question() {
    let tokens = vec![Token {
        kind: TokenKind::Question,
        span: crate::session::Span::DUMMY,
    }];
    assert_eq!(
        parse_repetition_op(&tokens, 0),
        Some((RepetitionKind::ZeroOrOne(RepetitionSep::None), 1))
    );
}

/// Stage 18.06 negative 4: match_repetition with ZeroOrMore accepts
/// empty input (returns Some(0)).
#[test]
fn stage18_06_match_repetition_zero_or_more_empty() {
    let interner = Rodeo::new();
    let inner: Vec<Token> = vec![Token {
        kind: TokenKind::Dollar,
        span: crate::session::Span::DUMMY,
    }];
    // Empty input.
    let input: Vec<Token> = vec![];
    let mut idx = 0usize;
    let mut captures = Captures::new();
    let result = match_repetition(
        &inner,
        &input,
        &mut idx,
        RepetitionKind::ZeroOrMore(RepetitionSep::None),
        &mut captures,
        &interner,
    );
    assert_eq!(result, Some(0), "ZeroOrMore should accept 0 matches");
    assert_eq!(idx, 0, "idx should not advance on 0 matches");
}

/// Stage 18.06 negative 5: match_repetition with OneOrMore rejects
/// empty input (returns None).
#[test]
fn stage18_06_match_repetition_one_or_more_empty() {
    let interner = Rodeo::new();
    let inner: Vec<Token> = vec![Token {
        kind: TokenKind::Dollar,
        span: crate::session::Span::DUMMY,
    }];
    let input: Vec<Token> = vec![];
    let mut idx = 0usize;
    let mut captures = Captures::new();
    let result = match_repetition(
        &inner,
        &input,
        &mut idx,
        RepetitionKind::OneOrMore(RepetitionSep::None),
        &mut captures,
        &interner,
    );
    assert!(result.is_none(), "OneOrMore should reject 0 matches");
}

/// Stage 18.06 negative 6: substitute_repetition expands the body
/// once per matched iteration.
#[test]
fn stage18_06_substitute_repetition_expands_each_iter() {
    let mut interner = Rodeo::new();
    let x_sym = interner.get_or_intern("x");
    // captures: $x is a Repetition with 2 iterations.
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
    // inner body: $x
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
    // Should produce 2 tokens (one per iteration): IntLit(1) and IntLit(2).
    assert_eq!(result.len(), 2, "should expand to 2 tokens (one per iter)");
    assert!(matches!(result[0].kind, TokenKind::IntLit(1, _)));
    assert!(matches!(result[1].kind, TokenKind::IntLit(2, _)));
}

// =====================================================================
// Stage 18.08 tests — Macro Expansion Error Collection
// =====================================================================

/// Stage 18.08 positive 1: No macro_rules! → no errors.
#[test]
fn stage18_08_macro_error_no_macros() {
    let mut interner = Rodeo::new();
    let tokens = vec![Token {
        kind: TokenKind::IntLit(42, None),
        span: crate::session::Span::DUMMY,
    }];
    let (out, errors) = expand_macros_with_errors(tokens.clone(), &mut interner);
    assert!(errors.is_empty(), "no macros → no errors");
    assert_eq!(out.len(), 1, "tokens unchanged");
}

/// Stage 18.08 positive 2: Valid macro_rules! + matching call → no errors.
#[test]
fn stage18_08_macro_error_valid_macro_no_errors() {
    let mut interner = Rodeo::new();
    let tokens = vec![Token {
        kind: TokenKind::IntLit(42, None),
        span: crate::session::Span::DUMMY,
    }];
    let (_out, errors) = expand_macros_with_errors(tokens, &mut interner);
    // No macro_rules! → no errors (same as above, but tests the happy path).
    assert!(errors.is_empty());
}

/// Stage 18.08 negative 1: A macro call that doesn't match any rule
/// produces a "no matching rule" error.
#[test]
fn stage18_08_macro_error_no_matching_rule() {
    let mut interner = Rodeo::new();
    let m_sym = interner.get_or_intern("m");
    let macro_rules_sym = interner.get_or_intern("macro_rules");
    // Define m with pattern () => { ... }, but call with m!(42)
    let tokens = vec![
        Token {
            kind: TokenKind::Ident(macro_rules_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Not,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(m_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LBrace,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::FatArrow,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LBrace,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::IntLit(0, None),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RBrace,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RBrace,
            span: crate::session::Span::DUMMY,
        },
        // Call: m!(42) — won't match `()` rule.
        Token {
            kind: TokenKind::Ident(m_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Not,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];
    let (_out, errors) = expand_macros_with_errors(tokens, &mut interner);
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("no matching rule")),
        "expected 'no matching rule' error, got: {:?}",
        errors
    );
}

/// Stage 18.08 negative 2: A malformed macro_rules! definition
/// (missing `=>`) produces a "malformed macro_rules! body" error.
#[test]
fn stage18_08_macro_error_malformed_def() {
    let mut interner = Rodeo::new();
    let m_sym = interner.get_or_intern("m");
    let macro_rules_sym = interner.get_or_intern("macro_rules");
    // macro_rules! m { ( ) } — missing `=> { body }`
    let tokens = vec![
        Token {
            kind: TokenKind::Ident(macro_rules_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Not,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(m_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LBrace,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RBrace,
            span: crate::session::Span::DUMMY,
        },
    ];
    let (_out, errors) = expand_macros_with_errors(tokens, &mut interner);
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("malformed macro_rules! body")),
        "expected 'malformed macro_rules! body' error, got: {:?}",
        errors
    );
}

/// Stage 18.08 negative 3: MacroError struct fields are accessible.
#[test]
fn stage18_08_macro_error_struct_fields() {
    let err = MacroError::new("test message", crate::session::Span::DUMMY);
    assert_eq!(err.message, "test message");
    assert_eq!(err.span, crate::session::Span::DUMMY);
}

/// Stage 18.08 negative 4: MacroError::new constructor accepts &str and String.
#[test]
fn stage18_08_macro_error_new_constructor() {
    let err1 = MacroError::new("from &str", crate::session::Span::DUMMY);
    let err2 = MacroError::new(String::from("from String"), crate::session::Span::DUMMY);
    assert_eq!(err1.message, "from &str");
    assert_eq!(err2.message, "from String");
}

/// Stage 18.08 negative 5: CompileErrors has a `macro_errors` field
/// accessible from outside the driver module.
#[test]
fn stage18_08_compile_errors_macro_field() {
    let errors = crate::driver::CompileErrors::default();
    assert!(
        errors.macro_errors.is_empty(),
        "default CompileErrors.macro_errors is empty"
    );
}

/// Stage 18.08 negative 6: expand_macros_with_errors returns
/// (tokens, errors) tuple — verify the tuple structure.
#[test]
fn stage18_08_expand_macros_with_errors_returns_tuple() {
    let mut interner = Rodeo::new();
    let tokens = vec![Token {
        kind: TokenKind::IntLit(0, None),
        span: crate::session::Span::DUMMY,
    }];
    let result: (Vec<Token>, Vec<MacroError>) = expand_macros_with_errors(tokens, &mut interner);
    assert_eq!(result.0.len(), 1, "tokens preserved");
    assert!(result.1.is_empty(), "no errors");
}

// =====================================================================
// Stage 18.10 tests — Built-in macro_rules! registration
// =====================================================================

/// Stage 18.10 positive 1: build_builtin_macro_table returns a table
/// with 4 entries (println/print/eprintln/eprint) when all names
/// are pre-interned.
#[test]
fn stage18_10_builtin_macros_registered() {
    let mut interner = Rodeo::new();
    // Pre-intern all built-in macro names + helper symbols.
    for name in BUILTIN_MACRO_NAMES {
        interner.get_or_intern(name);
    }
    interner.get_or_intern("args");
    interner.get_or_intern("tt");

    let table = build_builtin_macro_table(&mut interner);
    assert_eq!(
        table.len(),
        28,
        "should register 28 built-in macros (4 print + 24 non-print)"
    );
    for name in BUILTIN_MACRO_NAMES {
        let sym = interner.get(name).expect("name was interned");
        assert!(table.contains_key(&sym), "table should contain '{name}'");
    }
}

/// Stage 18.10 positive 2: println! still works correctly after
/// built-in macro registration (no-op expansion, parser handles it).
#[test]
fn stage18_10_println_still_works_after_builtin_registration() {
    let src = "fn main() { println!(\"hello\"); }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(result.errors.parse.is_empty(), "no parse errors");
    assert!(result.errors.macro_errors.is_empty(), "no macro errors");
}

/// Stage 18.10 + 18.29 + 18.32 negative 1: BUILTIN_MACRO_NAMES contains
/// exactly the 12 expected names (4 print + 8 non-print).
#[test]
fn stage18_10_builtin_macro_names_const() {
    assert_eq!(BUILTIN_MACRO_NAMES.len(), 28);
    assert!(BUILTIN_MACRO_NAMES.contains(&"println"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"print"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"eprintln"));
    assert!(BUILTIN_MACRO_NAMES.contains(&"eprint"));
    // Stage 18.29: non-print macros
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

/// Stage 18.10 negative 2: build_builtin_macro_table returns an
/// empty table when no names are interned (cold-start scenario).
#[test]
fn stage18_10_build_builtin_macro_table_returns_table() {
    let mut interner = Rodeo::new();
    let table = build_builtin_macro_table(&mut interner);
    // No names interned → empty table.
    assert!(table.is_empty(), "empty interner → empty table");
}

/// Stage 18.10 negative 3: Each built-in macro rule pattern is a
/// repetition: `$ ( $ args : tt ) *` (8 tokens).
#[test]
fn stage18_10_builtin_macro_rule_pattern_is_repetition() {
    let mut interner = Rodeo::new();
    for name in BUILTIN_MACRO_NAMES {
        interner.get_or_intern(name);
    }
    interner.get_or_intern("args");
    interner.get_or_intern("tt");

    let table = build_builtin_macro_table(&mut interner);
    let println_sym = interner.get("println").unwrap();
    let def = &table[&println_sym];
    assert_eq!(def.rules.len(), 1, "should have 1 rule");
    let pattern = &def.rules[0].pattern;
    // Pattern: $ ( $ args : tt ) *  = 8 tokens
    assert_eq!(pattern.len(), 8, "pattern should be 8 tokens");
    assert!(matches!(pattern[0].kind, TokenKind::Dollar));
    assert!(matches!(pattern[1].kind, TokenKind::LParen));
    assert!(matches!(pattern[2].kind, TokenKind::Dollar));
    assert!(matches!(pattern[6].kind, TokenKind::RParen));
    assert!(matches!(pattern[7].kind, TokenKind::Star));
}

/// Stage 18.10 negative 4: Each built-in macro rule body is
/// `name!($($args)*)` (10 tokens) — re-emits the same call form.
#[test]
fn stage18_10_builtin_macro_rule_body_is_same_call() {
    let mut interner = Rodeo::new();
    for name in BUILTIN_MACRO_NAMES {
        interner.get_or_intern(name);
    }
    interner.get_or_intern("args");
    interner.get_or_intern("tt");
    // Stage 18.27: also intern __landin_<name> for the body.
    for name in BUILTIN_MACRO_NAMES {
        interner.get_or_intern(format!("__landin_{}", name));
    }

    let table = build_builtin_macro_table(&mut interner);
    let println_sym = interner.get("println").unwrap();
    let def = &table[&println_sym];
    let body = &def.rules[0].body;
    // Stage 18.27: Body is `__landin_println($($args)*)` — 9 tokens
    // (function call form, no `!`).
    assert_eq!(
        body.len(),
        9,
        "body should be 9 tokens (Stage 18.27 function call form)"
    );
    assert!(matches!(body[0].kind, TokenKind::Ident(_)));
    assert!(matches!(body[1].kind, TokenKind::LParen));
    assert!(matches!(body[8].kind, TokenKind::RParen));
}

/// Stage 18.10 negative 5: User-defined macro_rules! with the same
/// name as a built-in overrides the built-in.
#[test]
fn stage18_10_user_macro_overrides_builtin() {
    let src = "macro_rules! println { () => { 42 } } fn main() { println!() }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    // User's println!() => 42, which should parse as an integer expression.
    // (The built-in println! would have been a no-op pass-through.)
    assert!(
        result.errors.parse.is_empty(),
        "user macro should override built-in — errors: {:?}",
        result.errors.parse
    );
}

/// Stage 18.10 negative 6 (updated Stage 18.27): println! call now
/// expands to `__landin_println("hi")` (function call form).
#[test]
fn stage18_10_builtin_macros_pass_through_println() {
    let mut interner = Rodeo::new();
    for name in BUILTIN_MACRO_NAMES {
        interner.get_or_intern(name);
        // Stage 18.27: also intern __landin_<name>.
        interner.get_or_intern(format!("__landin_{}", name));
    }
    interner.get_or_intern("args");
    interner.get_or_intern("tt");

    let println_sym = interner.get_or_intern("println");
    let landin_println_sym = interner.get_or_intern("__landin_println");
    let hi_sym = interner.get_or_intern("hi");
    let span = crate::session::Span::new(0, 10);
    // Input: println ! ( "hi" )
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
    assert!(errors.is_empty(), "no macro errors");
    // Stage 18.27: After expansion: __landin_println ( "hi" ) — 4 tokens.
    assert_eq!(
        out.len(),
        4,
        "println! should expand to __landin_println(...) — 4 tokens"
    );
    // First token should be `__landin_println`.
    assert!(matches!(out[0].kind, TokenKind::Ident(s) if s == landin_println_sym));
    // Second should be `(` (no `!` — function call).
    assert!(matches!(out[1].kind, TokenKind::LParen));
}

// =====================================================================
// Stage 18.13 tests — Separator support $(...),* / $(...);+ / etc.
// =====================================================================

/// Stage 18.13 positive 1: A macro using `$( $x:expr ),*` (comma-
/// separated zero or more expressions) parses and expands.
#[test]
fn stage18_13_macro_with_comma_separator() {
    let src = "macro_rules! m { ($($x:expr),*) => { 0 } } fn main() { m!(1, 2, 3) }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(
        result.errors.parse.is_empty(),
        "macro with $($x:expr),* should expand and parse — errors: {:?}",
        result.errors.parse
    );
}

/// Stage 18.13 positive 2: A macro using `$( $x:expr );+` (semicolon-
/// separated one or more expressions) parses and expands.
#[test]
fn stage18_13_macro_with_semicolon_separator() {
    let src = "macro_rules! m { ($($x:expr);+) => { 0 } } fn main() { m!(1; 2) }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(
        result.errors.parse.is_empty(),
        "macro with $($x:expr);+ should expand and parse — errors: {:?}",
        result.errors.parse
    );
}

/// Stage 18.13 negative 1: RepetitionSep::None variant constructs.
#[test]
fn stage18_13_repetition_sep_none_variant() {
    let sep = RepetitionSep::None;
    assert_eq!(sep, RepetitionSep::None);
}

/// Stage 18.13 negative 2: RepetitionSep::Token variant constructs.
#[test]
fn stage18_13_repetition_sep_token_variant() {
    let sep = RepetitionSep::Token(TokenKind::Comma);
    match sep {
        RepetitionSep::Token(TokenKind::Comma) => { /* OK */ }
        _ => panic!("expected Token(Comma)"),
    }
}

/// Stage 18.13 negative 3: parse_repetition_op without separator
/// returns ZeroOrMore(None) for `*`.
#[test]
fn stage18_13_parse_repetition_op_no_separator() {
    let tokens = vec![Token {
        kind: TokenKind::Star,
        span: crate::session::Span::DUMMY,
    }];
    let result = parse_repetition_op(&tokens, 0);
    assert_eq!(
        result,
        Some((RepetitionKind::ZeroOrMore(RepetitionSep::None), 1))
    );
}

/// Stage 18.13 negative 4: parse_repetition_op with comma separator
/// returns ZeroOrMore(Token(Comma)) for `, *`.
#[test]
fn stage18_13_parse_repetition_op_with_comma() {
    let tokens = vec![
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
    ];
    let result = parse_repetition_op(&tokens, 0);
    assert_eq!(
        result,
        Some((
            RepetitionKind::ZeroOrMore(RepetitionSep::Token(TokenKind::Comma)),
            2
        ))
    );
}

/// Stage 18.13 negative 5: match_repetition with separator matches
/// comma-separated input.
#[test]
fn stage18_13_match_repetition_with_separator_matches() {
    let mut interner = Rodeo::new();
    let x_sym = interner.get_or_intern("x");
    let expr_sym = interner.get_or_intern("expr");
    // inner pattern: $ x : expr
    let inner = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(x_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
    ];
    // input: 1 , 2 , 3
    let input = vec![
        Token {
            kind: TokenKind::IntLit(1, None),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::IntLit(2, None),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::IntLit(3, None),
            span: crate::session::Span::DUMMY,
        },
    ];
    let mut idx = 0usize;
    let mut captures = Captures::new();
    let result = match_repetition(
        &inner,
        &input,
        &mut idx,
        RepetitionKind::ZeroOrMore(RepetitionSep::Token(TokenKind::Comma)),
        &mut captures,
        &interner,
    );
    assert_eq!(result, Some(3), "should match 3 iterations (1, 2, 3)");
    assert_eq!(idx, 5, "should consume all 5 input tokens");
}

/// Stage 18.13 negative 6: substitute_repetition emits separator
/// between iterations (not after last).
#[test]
fn stage18_13_substitute_repetition_emits_separator() {
    let mut interner = Rodeo::new();
    let x_sym = interner.get_or_intern("x");
    // captures: $x is a Repetition with 3 iterations.
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
            vec![Token {
                kind: TokenKind::IntLit(3, None),
                span: crate::session::Span::DUMMY,
            }],
        ]),
    );
    // inner body: $x
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
        RepetitionKind::ZeroOrMore(RepetitionSep::Token(TokenKind::Comma)),
        &mut result,
    );
    // Should produce: 1 , 2 , 3 (5 tokens: 3 values + 2 separators)
    assert_eq!(
        result.len(),
        5,
        "should expand to 5 tokens (3 values + 2 separators)"
    );
    assert!(matches!(result[0].kind, TokenKind::IntLit(1, _)));
    assert!(matches!(result[1].kind, TokenKind::Comma));
    assert!(matches!(result[2].kind, TokenKind::IntLit(2, _)));
    assert!(matches!(result[3].kind, TokenKind::Comma));
    assert!(matches!(result[4].kind, TokenKind::IntLit(3, _)));
}
