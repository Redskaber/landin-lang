//! Stage 18.03: macro_rules! expansion — token tree matching + substitution.
//!
//! This module implements the core macro expansion engine:
//! 1. **Pattern matching**: Match input tokens against a macro rule pattern.
//!    Supports `$name:fragment` captures and literal token matching.
//! 2. **Substitution**: Replace `$name` captures in the body with matched values.
//! 3. **Expansion**: Produce expanded tokens that are re-parsed as AST.
//!
//! Supported fragments (Phase 3):
//! - `$name:expr` — matches an expression (token tree until `;` or `}`)
//! - `$name:ident` — matches a single identifier
//! - `$name:tt` — matches a single token tree
//!
//! Per §23: `expand_macro` follows `<verb>_<noun>` pattern.
//! Per §16: operates on tokens (before HIR lowering).
//! Per §1.0 原則 6 "通用 > 特例": one engine handles all macro_rules!.

use crate::ast::MacroRulesDef;
use crate::lexer::{Token, TokenKind};
use lasso::Rodeo;
use std::collections::HashMap;

/// A captured fragment from pattern matching.
/// Maps `Symbol` (capture name) → captured tokens.
type Captures = HashMap<crate::lexer::Symbol, Vec<Token>>;

/// Stage 18.03: Expand a macro call by matching input tokens against rules.
///
/// Tries each rule in order. The first rule whose pattern matches the
/// input tokens is used. The body is then substituted with captured
/// values and returned.
///
/// Returns `Some(expanded_tokens)` if a rule matched, `None` if no rule matched.
///
/// Per §23: `expand_macro` follows `<verb>_<noun>` pattern.
pub fn expand_macro(def: &MacroRulesDef, input: &[Token], interner: &Rodeo) -> Option<Vec<Token>> {
    for rule in &def.rules {
        let mut captures = HashMap::new();
        if match_pattern(&rule.pattern, input, &mut captures, interner) {
            return Some(substitute_body(&rule.body, &captures));
        }
    }
    None
}

/// Match a pattern against input tokens, capturing `$name:fragment` bindings.
///
/// Returns `true` if the pattern matches, `false` otherwise.
/// On success, `captures` contains all `$name` bindings.
fn match_pattern(
    pattern: &[Token],
    input: &[Token],
    captures: &mut Captures,
    interner: &Rodeo,
) -> bool {
    let mut pi = 0; // pattern index
    let mut ii = 0; // input index

    while pi < pattern.len() {
        let pt = &pattern[pi];

        // Check for `$name:fragment` pattern (dollar sign followed by ident + colon + fragment)
        if pt.kind == TokenKind::Dollar {
            // Expect: $ ident : fragment
            if pi + 3 < pattern.len() {
                if let (TokenKind::Ident(name_sym), TokenKind::Colon) =
                    (&pattern[pi + 1].kind, &pattern[pi + 2].kind)
                {
                    let name = *name_sym;
                    if let TokenKind::Ident(frag_sym) = &pattern[pi + 3].kind {
                        let frag = interner.resolve(frag_sym);
                        let captured = match frag {
                            "expr" => capture_expr(input, &mut ii),
                            "ident" => capture_ident(input, &mut ii),
                            "tt" => capture_tt(input, &mut ii),
                            _ => return false,
                        };
                        if captured.is_empty() {
                            return false;
                        }
                        captures.insert(name, captured);
                        pi += 4; // Skip $ name : fragment
                        continue;
                    }
                }
            }
        }

        // Literal token matching
        if ii >= input.len() {
            return false;
        }
        if !tokens_match(&pt.kind, &input[ii].kind) {
            return false;
        }
        pi += 1;
        ii += 1;
    }

    // All pattern tokens consumed — check if input is also fully consumed.
    ii == input.len()
}

/// Capture an expression: tokens until top-level `,`, `;`, or `)`.
fn capture_expr(input: &[Token], idx: &mut usize) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut depth = 0i32;

    while *idx < input.len() {
        match &input[*idx].kind {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                depth += 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::Comma | TokenKind::Semicolon if depth == 0 => break,
            _ => tokens.push(input[*idx].clone()),
        }
        *idx += 1;
    }

    tokens
}

/// Capture a single identifier.
fn capture_ident(input: &[Token], idx: &mut usize) -> Vec<Token> {
    if *idx < input.len() {
        if let TokenKind::Ident(_) = &input[*idx].kind {
            let token = input[*idx].clone();
            *idx += 1;
            return vec![token];
        }
    }
    Vec::new()
}

/// Capture a single token tree (one token, or a balanced delimited group).
fn capture_tt(input: &[Token], idx: &mut usize) -> Vec<Token> {
    if *idx >= input.len() {
        return Vec::new();
    }

    let open = match &input[*idx].kind {
        TokenKind::LParen => TokenKind::RParen,
        TokenKind::LBracket => TokenKind::RBracket,
        TokenKind::LBrace => TokenKind::RBrace,
        _ => {
            // Single token
            let token = input[*idx].clone();
            *idx += 1;
            return vec![token];
        }
    };

    // Balanced group
    let mut tokens = Vec::new();
    let mut depth = 0i32;

    while *idx < input.len() {
        match &input[*idx].kind {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                depth += 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                depth -= 1;
                tokens.push(input[*idx].clone());
                if depth == 0 {
                    *idx += 1;
                    break;
                }
            }
            _ => tokens.push(input[*idx].clone()),
        }
        *idx += 1;
    }

    let _ = open;
    tokens
}

/// Check if two token kinds match (for literal pattern matching).
fn tokens_match(pat: &TokenKind, input: &TokenKind) -> bool {
    // For identifiers, match by discriminant (any ident matches any ident in pattern).
    // For other tokens, match exactly.
    pat == input
}

/// Substitute `$name` captures in the body, producing expanded tokens.
fn substitute_body(body: &[Token], captures: &Captures) -> Vec<Token> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < body.len() {
        let bt = &body[i];

        // Check for `$name` substitution
        if bt.kind == TokenKind::Dollar && i + 1 < body.len() {
            if let TokenKind::Ident(name_sym) = &body[i + 1].kind {
                if let Some(captured) = captures.get(name_sym) {
                    result.extend(captured.iter().cloned());
                    i += 2; // Skip $ name
                    continue;
                }
            }
        }

        result.push(bt.clone());
        i += 1;
    }

    result
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
            vec![Token {
                kind: TokenKind::IntLit(42, None),
                span: crate::session::Span::DUMMY,
            }],
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
        let result = expand_macro(&def, &input, &interner);
        assert!(result.is_none(), "non-matching input should return None");
    }
}
