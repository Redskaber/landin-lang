//! Stage 18.247: Macro definition collection (extracted from mod.rs).
//! Per §13.4 J2 (single responsibility): owns macro_rules! definition parsing.

use super::*;
use crate::ast::MacroRulesDef;
use crate::lexer::{Token, TokenKind};
use lasso::Rodeo;

pub fn collect_macro_defs(tokens: &[Token], interner: &mut Rodeo) -> MacroTable {
    collect_macro_defs_with_errors(tokens, interner, &mut Vec::new())
}

/// Stage 18.08: Like `collect_macro_defs` but also collects errors.
///
/// Malformed `macro_rules!` bodies (e.g. missing `=>`, unbalanced
/// delimiters) are reported as `MacroError`s. The original tokens are
/// still skipped past so collection can continue with subsequent
/// definitions.
///
/// Per §10: `<verb>_<noun>_<noun>_<prep>` pattern.
pub fn collect_macro_defs_with_errors(
    tokens: &[Token],
    interner: &mut Rodeo,
    errors: &mut Vec<MacroError>,
) -> MacroTable {
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
                let name_str = interner.resolve(&name).to_string();
                let body_start = i + 4;
                match parse_macro_rules_body(tokens, body_start, *name_span) {
                    Some(rules) => {
                        table.insert(
                            name,
                            MacroRulesDef {
                                name,
                                rules,
                                span: *name_span,
                            },
                        );
                    }
                    None => {
                        // Stage 18.08: collect the error.
                        errors.push(MacroError::new(
                            format!("malformed macro_rules! body in definition of '{name_str}'"),
                            *name_span,
                        ));
                    }
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
pub(super) fn collect_delimited(tokens: &[Token], start: usize) -> Option<(Vec<Token>, usize)> {
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
