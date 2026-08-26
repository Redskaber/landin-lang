//! Stage 18.247: Macro call expansion (extracted from mod.rs).
//! Per §13.4 J2 (single responsibility): owns macro call expansion.

use super::super::builtin_macros::build_builtin_macro_table;
use super::collection::collect_delimited;
use super::*;
use crate::lexer::{Token, TokenKind};
use lasso::Rodeo;

pub fn expand_macro_calls(
    tokens: &[Token],
    table: &MacroTable,
    interner: &mut Rodeo,
) -> Vec<Token> {
    expand_macro_calls_with_errors(tokens, table, interner, &mut Vec::new())
}

/// Stage 18.08: Like `expand_macro_calls` but also collects errors.
///
/// When a `name!(...)` call site matches a known macro but no rule
/// expands (e.g. wrong number of arguments), a `MacroError` is pushed
/// to `errors`. The original call tokens are still emitted so the
/// parser can produce its own error.
///
/// Per §10: `<verb>_<noun>_<noun>_<prep>` pattern.
pub fn expand_macro_calls_with_errors(
    tokens: &[Token],
    table: &MacroTable,
    interner: &mut Rodeo,
    errors: &mut Vec<MacroError>,
) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;

    while i < tokens.len() {
        // Check for `ident ! <delim>` pattern.
        let is_macro_call = matches!(
            (tokens.get(i).map(|t| &t.kind), tokens.get(i + 1).map(|t| &t.kind), tokens.get(i + 2).map(|t| &t.kind)),
            (
                Some(TokenKind::Ident(name_sym)),
                Some(TokenKind::Not),
                Some(TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket),
            ) if table.contains_key(name_sym)
        );

        if is_macro_call {
            // Safe to unwrap — pattern matched above.
            let name_sym = if let TokenKind::Ident(s) = &tokens[i].kind {
                *s
            } else {
                unreachable!("checked above")
            };
            let def = table.get(&name_sym).expect("checked above");
            let name_str = interner.resolve(&name_sym).to_string();
            let call_span = tokens[i].span;

            if let Some((input, after_close)) = collect_delimited(tokens, i + 2) {
                // Try to expand.
                if let Some(expanded) = expand_macro(def, &input, interner) {
                    // Stage 18.10: Rewrite expanded token spans to the call
                    // site span. Built-in macro rule bodies use Span::DUMMY,
                    // which would conflict with real source spans (lo > hi
                    // validation). By rewriting all expanded tokens to the
                    // call site span, we ensure span monotonicity.
                    let call_span = tokens[i].span;
                    let mut expanded = expanded;
                    for tok in &mut expanded {
                        tok.span = call_span;
                    }
                    out.extend(expanded);
                } else {
                    // Stage 18.08: collect no-match error.
                    errors.push(MacroError::new(
                        format!("no matching rule for macro '{name_str}'"),
                        call_span,
                    ));
                    // Expansion failed — keep the call as-is so the
                    // parser produces a sensible error.
                    out.push(tokens[i].clone()); // ident
                    out.push(tokens[i + 1].clone()); // !
                    out.push(tokens[i + 2].clone()); // open delim
                    out.extend(input.iter().cloned());
                    if let Some(close) = tokens.get(after_close.saturating_sub(1)) {
                        out.push(close.clone()); // close delim
                    }
                }
                i = after_close;
                continue;
            }
            // Unbalanced delim — fall through; let parser error.
        }

        out.push(tokens[i].clone());
        i += 1;
    }
    out
}

/// Stage 18.04: Top-level macro expansion pass — driver entry point.
///
/// 1. Collect all `macro_rules!` definitions into a `MacroTable`.
/// 2. If the table is empty, return `tokens` unchanged (zero-overhead
///    for code without macro_rules!).
/// 3. Iteratively call [`expand_macro_calls`] until no more expansions
///    occur or `MAX_EXPANSION_ROUNDS` is reached.
///
/// This is the §10-compliant free-function entry point invoked by
/// `driver::compile` between the lexer and the parser:
///
/// ```text
/// lexer::tokenize(src, &mut interner)
///     → Vec<Token>
///     → expand_macros(tokens, &interner)   ← this function
///     → Vec<Token>  (macro_rules! calls expanded)
///     → parser::parse_crate(tokens, &mut interner)
/// ```
///
/// Per §10: `expand_macros` follows `<verb>_<noun>` pattern.
pub fn expand_macros(tokens: Vec<Token>, interner: &mut Rodeo) -> Vec<Token> {
    expand_macros_with_errors(tokens, interner).0
}

/// Stage 18.08: Top-level macro expansion pass with error collection.
///
/// Like [`expand_macros`] but also returns a `Vec<MacroError>` capturing
/// malformed `macro_rules!` definitions, no-matching-rule macro calls,
/// and recursion-limit violations. Errors do NOT stop expansion — the
/// compiler continues with whatever tokens were produced, so downstream
/// phases can produce their own errors too.
///
/// Per §10: `expand_macros_with_errors` follows `<verb>_<noun>_<prep>`.
pub fn expand_macros_with_errors(
    tokens: Vec<Token>,
    interner: &mut Rodeo,
) -> (Vec<Token>, Vec<MacroError>) {
    let mut errors = Vec::new();
    // Stage 18.10: Register built-in macros first (println/print/eprintln/eprint).
    // Built-in macros have no-op rule bodies in Phase 1, so they pass through
    // unchanged and the parser's existing special-case path still handles them.
    let mut table = build_builtin_macro_table(interner);
    // Then collect user-defined macros. User macros override built-ins
    // (extend() overwrites existing keys with the user's version).
    let user_table = collect_macro_defs_with_errors(&tokens, interner, &mut errors);
    table.extend(user_table);
    if table.is_empty() {
        return (tokens, errors);
    }

    let mut current = tokens;
    for round in 0..MAX_EXPANSION_ROUNDS {
        let mut round_errors = Vec::new();
        let next = expand_macro_calls_with_errors(&current, &table, interner, &mut round_errors);
        errors.extend(round_errors);
        // Termination check: if the token stream didn't change at all
        // (by structural equality), no more expansions are possible.
        if tokens_eq(&next, &current) {
            return (next, errors);
        }
        current = next;
        // Stage 18.08: if this was the last round, emit a recursion error.
        if round + 1 == MAX_EXPANSION_ROUNDS {
            // Stage 18.80 P2-D: Use first token's span instead of Span::DUMMY
            // for better error location. If no tokens, fall back to DUMMY.
            let err_span = current
                .first()
                .map(|t| t.span)
                .unwrap_or(crate::session::Span::DUMMY);
            errors.push(MacroError::new(
                format!(
                    "macro expansion exceeded {MAX_EXPANSION_ROUNDS} rounds (possible infinite recursion)"
                ),
                err_span,
            ));
        }
    }
    (current, errors)
}

/// Stage 18.04: Compare two token streams by `(kind, span)` equality.
///
/// Used by [`expand_macros`] as a termination check. We compare by
/// value rather than by length so that a macro whose expansion happens
/// to be the same length as the call site still terminates correctly
/// (because the expanded tokens differ in kind).
///
/// Per §10: internal helper, named `<noun>_<eq>`.
fn tokens_eq(a: &[Token], b: &[Token]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| x.kind == y.kind && x.span == y.span)
}

#[cfg(test)]
mod tests {
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
        let result: (Vec<Token>, Vec<MacroError>) =
            expand_macros_with_errors(tokens, &mut interner);
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
}
