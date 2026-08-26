//! Stage 18.247: Macro call expansion (extracted from mod.rs).
//! Stage 18.248: Tests extracted to expansion_tests.rs.
//!
//! Per §13.4 J2 (single responsibility): owns macro call expansion.
//! Per §1.0 原則 6 (通解 > 特解): one expansion path for all macro calls.

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
#[path = "expansion_tests.rs"]
mod tests;
