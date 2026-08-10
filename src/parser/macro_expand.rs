//! Stage 18.03 + 18.04: macro_rules! expansion engine.
//!
//! This module implements the macro_rules! system:
//!
//! # Stage 18.03 — token tree matching + substitution
//! 1. **Pattern matching**: Match input tokens against a macro rule pattern.
//!    Supports `$name:fragment` captures and literal token matching.
//! 2. **Substitution**: Replace `$name` captures in the body with matched values.
//! 3. **Expansion**: Produce expanded tokens that are re-parsed as AST.
//!
//! Supported fragments (Stage 18.03):
//! - `$name:expr` — matches an expression (token tree until `;` or `}`)
//! - `$name:ident` — matches a single identifier
//! - `$name:tt` — matches a single token tree
//!
//! # Stage 18.04 — macro call invocation + driver integration
//! 4. **collect_macro_defs**: Scan a token stream and extract every
//!    `macro_rules! name { ... }` definition into a `MacroTable`.
//! 5. **expand_macro_calls**: Walk a token stream and expand every
//!    `name!(args)` call site whose `name` is in the table.
//! 6. **expand_macros**: Top-level driver entry — collect defs, then
//!    iteratively expand calls until no more expansions occur.
//!
//! Per §10: `expand_macros` is the free-function entry point
//! (`<verb>_<noun>` pattern); `collect_macro_defs` and
//! `expand_macro_calls` follow `<verb>_<noun>_<noun>`.
//! Per §11: the entire module is parser-internal; driver only sees
//! the `expand_macros` free-function entry.
//! Per §1.0 原則 6 "通用 > 特例": one engine handles all macro_rules!.

use crate::ast::{MacroRule, MacroRulesDef};
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

// =============================================================================
// Stage 18.04: Macro Call Invocation + Driver Integration
// =============================================================================

/// Stage 18.04: Maximum number of expansion rounds to prevent infinite
/// recursion when a macro expands to a call to itself.
///
/// 32 is sufficient for realistic macro chains (real code rarely nests
/// more than 5-10 levels deep) while providing a hard safety bound.
const MAX_EXPANSION_ROUNDS: usize = 32;

/// Stage 18.04: Macro definition table collected from a token stream.
///
/// Maps macro name (`Symbol`) → `MacroRulesDef` (with parsed rules).
/// Built by [`collect_macro_defs`] and consumed by
/// [`expand_macro_calls`].
///
/// Per §10: type name follows the `<Noun>` + `Table` suffix convention
/// (mirrors `FnSigTable`, `FieldTyTable`).
pub type MacroTable = HashMap<crate::lexer::Symbol, MacroRulesDef>;

/// Stage 18.04: Collect all `macro_rules!` definitions from a token stream.
///
/// Walks the token stream looking for the pattern:
/// ```text
/// Ident("macro_rules") Bang Ident(name) LBrace ... RBrace
/// ```
/// For each match, parses the rule bodies `(pat) => { body };` into
/// `MacroRule`s and stores the resulting `MacroRulesDef` in the table.
///
/// The original tokens are **not** modified — the parser will still
/// parse `macro_rules!` definitions normally and produce
/// `ItemKind::MacroRules` AST nodes. This function only builds a
/// lookup table for the pre-parse expansion pass.
///
/// Per §10: `collect_macro_defs` follows `<verb>_<noun>_<noun>` pattern.
pub fn collect_macro_defs(tokens: &[Token], interner: &Rodeo) -> MacroTable {
    let mut table = MacroTable::new();
    let macro_rules_sym = match interner.get("macro_rules") {
        Some(s) => s,
        None => return table,
    };

    let mut i = 0;
    while i < tokens.len() {
        // Match: Ident("macro_rules") Bang Ident(name) LBrace
        if let (
            Some(Token {
                kind: TokenKind::Ident(sym),
                ..
            }),
            Some(Token {
                kind: TokenKind::Not,
                ..
            }),
            Some(Token {
                kind: TokenKind::Ident(name_sym),
                span: name_span,
            }),
            Some(Token {
                kind: TokenKind::LBrace,
                ..
            }),
        ) = (
            tokens.get(i),
            tokens.get(i + 1),
            tokens.get(i + 2),
            tokens.get(i + 3),
        ) {
            if *sym == macro_rules_sym {
                let name = *name_sym;
                let body_start = i + 4;
                if let Some(rules) = parse_macro_rules_body(tokens, body_start, *name_span) {
                    table.insert(
                        name,
                        MacroRulesDef {
                            name,
                            rules,
                            span: *name_span,
                        },
                    );
                }
                // Skip past the macro_rules! definition (past matching `}`).
                // Even if parsing failed, we must skip to avoid re-processing.
                i = skip_to_matching_rbrace(tokens, body_start);
                continue;
            }
        }
        i += 1;
    }
    table
}

/// Stage 18.04: Parse the body of a `macro_rules! name { ... }` definition.
///
/// The body is a sequence of rules: `(pattern) => { body };` (possibly
/// with `[`/`{` delimiters instead of `(` for the pattern).
///
/// `start` is the index just after the opening `{` of the macro_rules! body.
/// Returns `Some(Vec<MacroRule>)` if at least one rule parsed successfully,
/// or `None` if the body could not be parsed.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn parse_macro_rules_body(
    tokens: &[Token],
    start: usize,
    span: crate::session::Span,
) -> Option<Vec<MacroRule>> {
    let mut rules = Vec::new();
    let mut i = start;

    while i < tokens.len() {
        // Skip trailing `;` between rules.
        if let Some(Token {
            kind: TokenKind::Semicolon,
            ..
        }) = tokens.get(i)
        {
            i += 1;
            continue;
        }
        // End of macro_rules! body.
        if let Some(Token {
            kind: TokenKind::RBrace,
            ..
        }) = tokens.get(i)
        {
            break;
        }

        // Parse pattern: a delimited token tree.
        let (pattern, after_pattern) = match collect_delimited(tokens, i) {
            Some(x) => x,
            None => break,
        };
        i = after_pattern;

        // Expect `=>`
        if !matches!(tokens.get(i).map(|t| &t.kind), Some(TokenKind::FatArrow)) {
            break;
        }
        i += 1; // consume `=>`

        // Parse body: a delimited token tree.
        let (body, after_body) = match collect_delimited(tokens, i) {
            Some(x) => x,
            None => break,
        };
        i = after_body;

        rules.push(MacroRule {
            pattern,
            body,
            span,
        });
    }

    if rules.is_empty() {
        None
    } else {
        Some(rules)
    }
}

/// Stage 18.04: Collect a balanced delimited token tree starting at `start`.
///
/// Returns `(tokens_inside, index_after_closing_delim)`. The returned
/// tokens do **not** include the opening/closing delimiters themselves
/// (matching the existing `MacroRule.pattern` / `MacroRule.body`
/// conventions from Stage 18.02).
///
/// Returns `None` if `start` doesn't point at an opening delimiter or
/// if the stream ends before the matching closer.
///
/// Per §10: internal helper, named `<verb>_<noun>`.
fn collect_delimited(tokens: &[Token], start: usize) -> Option<(Vec<Token>, usize)> {
    let open_kind = &tokens.get(start)?.kind;
    if !matches!(
        open_kind,
        TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket
    ) {
        return None;
    }

    let mut depth = 0i32;
    let mut inside = Vec::new();
    let mut i = start;

    while i < tokens.len() {
        let tok = &tokens[i];
        match &tok.kind {
            TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket => {
                depth += 1;
                // Skip the outermost opening delim; include nested ones.
                if i != start {
                    inside.push(tok.clone());
                }
            }
            TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket => {
                depth -= 1;
                if depth == 0 {
                    // Closing the outermost delim.
                    return Some((inside, i + 1));
                }
                inside.push(tok.clone());
            }
            TokenKind::Eof => return None,
            _ => {
                inside.push(tok.clone());
            }
        }
        i += 1;
    }
    None
}

/// Stage 18.04: Find the index just past the matching `}` for the `{`
/// (or `)` for `(`, etc.) that **should** be at `start`.
///
/// Used by [`collect_macro_defs`] to skip past a `macro_rules!` body
/// even when individual rule parsing fails.
///
/// Per §10: internal helper, named `<verb>_<preposition>_<noun>_<noun>`.
fn skip_to_matching_rbrace(tokens: &[Token], start: usize) -> usize {
    let mut depth = 0i32;
    let mut i = start;
    while i < tokens.len() {
        match &tokens[i].kind {
            TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket => depth += 1,
            TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket => {
                depth -= 1;
                if depth <= 0 {
                    return i + 1;
                }
            }
            TokenKind::Eof => return i,
            _ => {}
        }
        i += 1;
    }
    i
}

/// Stage 18.04: Expand macro calls in a token stream.
///
/// Walks `tokens` looking for the pattern `Ident(name) Bang <delim>`.
/// If `name` is in `table`, collects the input tokens until the matching
/// closing delimiter, calls [`expand_macro`] on them, and splices the
/// expanded tokens into the output. If expansion fails (no rule matched),
/// the original call tokens are emitted unchanged.
///
/// Unknown macros (e.g. `println!`, which is a built-in special form)
/// are passed through unchanged so the parser can handle them via its
/// existing special cases.
///
/// Per §10: `expand_macro_calls` follows `<verb>_<noun>_<noun>` pattern.
pub fn expand_macro_calls(tokens: &[Token], table: &MacroTable, interner: &Rodeo) -> Vec<Token> {
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

            if let Some((input, after_close)) = collect_delimited(tokens, i + 2) {
                // Try to expand.
                if let Some(expanded) = expand_macro(def, &input, interner) {
                    out.extend(expanded);
                } else {
                    // Expansion failed — keep the call as-is so the
                    // parser produces a sensible error.
                    out.push(tokens[i].clone()); // ident
                    out.push(tokens[i + 1].clone()); // !
                    out.push(tokens[i + 2].clone()); // open delim
                    out.extend(input);
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
pub fn expand_macros(tokens: Vec<Token>, interner: &Rodeo) -> Vec<Token> {
    let table = collect_macro_defs(&tokens, interner);
    if table.is_empty() {
        return tokens;
    }

    let mut current = tokens;
    for _ in 0..MAX_EXPANSION_ROUNDS {
        let next = expand_macro_calls(&current, &table, interner);
        // Termination check: if the token stream didn't change at all
        // (by structural equality), no more expansions are possible.
        if tokens_eq(&next, &current) {
            return next;
        }
        current = next;
    }
    current
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
        let interner = Rodeo::new();
        let tokens: Vec<Token> = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let table = collect_macro_defs(&tokens, &interner);
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
        let table = collect_macro_defs(&tokens, &interner);
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
        let out = expand_macro_calls(&tokens, &table, &interner);
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
        let interner = Rodeo::new();
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
        let out = expand_macro_calls(&tokens, &table, &interner);
        assert_eq!(out.len(), 3, "no macros → 3 tokens unchanged");
    }

    /// Stage 18.04 negative 5: expand_macros with no macro_rules! defs
    /// returns the input tokens unchanged (zero-overhead fast path).
    #[test]
    fn stage18_04_expand_macros_no_macros_returns_input() {
        let interner = Rodeo::new();
        let tokens = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let out = expand_macros(tokens.clone(), &interner);
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
}
