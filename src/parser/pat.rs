//! Stage 6.12 (TD-022): Pattern parsing sub-module.
//!
//! Per 02-grammar.md §3.5 (Pattern). Extracted from `parser.rs`
//! per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4.
//!
//! Owns:
//! - `parse_pat` (top-level pattern — or-pattern)
//! - `parse_or_pat` (`pat | pat | pat`)
//! - `parse_pat_no_or` (single pattern, no `|`)
//! - `skip_delim_group` (helper for skipping `(...)`/`{...}`/`[...]`)

use crate::ast::*;
use crate::lexer::token::*;

use super::parser::Parser;

impl<'a> Parser<'a> {
    // --- Patterns ---

    /// Parse a pattern. Per 02-grammar.md §3.5, patterns are:
    ///   `_` | ident | mut ident | ref ident | ref mut ident
    ///   | lit | path | path(args) | path{ fields }
    ///   | &(mut)? pat | pat @ pat
    ///   | (pat, pat, ...) | [pat, ...]
    ///   | pat | pat (or-pattern, lowest precedence)
    ///   | pat .. pat | pat ..= pat | ..pat | pat..
    ///
    /// Top-level `parse_pat` parses an or-pattern (the lowest-precedence form).
    pub(super) fn parse_pat(&mut self) -> Pat {
        self.parse_or_pat()
    }

    /// Parse an or-pattern: `pat | pat | pat`.
    pub(super) fn parse_or_pat(&mut self) -> Pat {
        let first = self.parse_pat_no_or();
        if *self.peek() != TokenKind::Or {
            return first;
        }
        let span = self.current_span();
        let mut pats = vec![first];
        while *self.peek() == TokenKind::Or {
            self.bump();
            pats.push(self.parse_pat_no_or());
        }
        Pat::Or(pats, span)
    }

    /// Parse a single pattern (no `|`).
    pub(super) fn parse_pat_no_or(&mut self) -> Pat {
        let span = self.current_span();
        match self.peek().clone() {
            TokenKind::Underscore => {
                self.bump();
                Pat::Wild(span)
            }
            TokenKind::KwMut => {
                self.bump();
                let ident = self.expect_ident("pattern binding name");
                Pat::Ident(BindingMode::ByValue(Mutability::Mutable), ident, None)
            }
            TokenKind::KwRef => {
                self.bump();
                let mutability = if *self.peek() == TokenKind::KwMut {
                    self.bump();
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                let ident = self.expect_ident("pattern binding name");
                Pat::Ident(BindingMode::ByRef(mutability), ident, None)
            }
            TokenKind::And => {
                self.bump();
                let mutability = if *self.peek() == TokenKind::KwMut {
                    self.bump();
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                let pat = self.parse_pat_no_or();
                Pat::Ref(Box::new(pat), mutability, span)
            }
            // Literal patterns: integers, floats, chars, strings, bools
            TokenKind::IntLit(_, _)
            | TokenKind::FloatLit(_, _)
            | TokenKind::CharLit(_)
            | TokenKind::StrLit(_)
            | TokenKind::ByteLit(_)
            | TokenKind::ByteStrLit(_)
            | TokenKind::KwTrue
            | TokenKind::KwFalse
            | TokenKind::Minus => {
                // Negative literal patterns like `-1`
                let expr = self.parse_primary_expr();
                // Range pattern: `1..10` or `1..=10`
                if *self.peek() == TokenKind::DotDot || *self.peek() == TokenKind::DotDotEq {
                    let end_kind = if *self.peek() == TokenKind::DotDotEq {
                        RangeEnd::Included
                    } else {
                        RangeEnd::Excluded
                    };
                    self.bump();
                    let end = if matches!(
                        self.peek(),
                        TokenKind::IntLit(_, _)
                            | TokenKind::FloatLit(_, _)
                            | TokenKind::CharLit(_)
                            | TokenKind::KwTrue
                            | TokenKind::KwFalse
                            | TokenKind::Minus
                    ) {
                        Some(Box::new(self.parse_primary_expr()))
                    } else {
                        None
                    };
                    return Pat::Range(Some(Box::new(expr)), end, end_kind, span);
                }
                Pat::Lit(Box::new(expr))
            }
            TokenKind::LParen => {
                // Tuple pattern: (a, b, c)
                self.bump();
                let mut pats = Vec::new();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    pats.push(self.parse_pat());
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen, "`)`");
                Pat::Tuple(pats, span)
            }
            TokenKind::LBracket => {
                // Slice pattern: [a, b, .., c]
                self.bump();
                let mut pats = Vec::new();
                let mut rest = None;
                while !matches!(self.peek(), TokenKind::RBracket | TokenKind::Eof) {
                    if *self.peek() == TokenKind::DotDot || *self.peek() == TokenKind::DotDotEq {
                        let _r_span = self.current_span();
                        self.bump();
                        // `..` is a Rest; `..pat` binds the rest to a sub-pattern.
                        // We only consume a sub-pattern if the next token can start one.
                        if matches!(
                            self.peek(),
                            TokenKind::Underscore
                                | TokenKind::Ident(_)
                                | TokenKind::RawIdent(_)
                                | TokenKind::KwMut
                                | TokenKind::KwRef
                                | TokenKind::And
                                | TokenKind::LParen
                                | TokenKind::LBracket
                        ) {
                            rest = Some(Box::new(self.parse_pat()));
                        } else {
                            // Bare `..` — no sub-pattern
                            rest = Some(Box::new(Pat::Wild(self.current_span())));
                        }
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                        continue;
                    }
                    pats.push(self.parse_pat());
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RBracket, "`]`");
                Pat::Slice(pats, rest, span)
            }
            // Identifier or path-based pattern
            TokenKind::Ident(_)
            | TokenKind::RawIdent(_)
            | TokenKind::KwSelf_
            | TokenKind::KwSelfType
            | TokenKind::KwCrate
            | TokenKind::KwSuper
            | TokenKind::PathSep => {
                // Try path pattern (might be TupleStruct or Struct)
                let path = self.parse_path_in_pat();
                let pat = match self.peek() {
                    TokenKind::LParen => {
                        // Tuple struct pattern: Path(pat, pat, ...)
                        self.bump();
                        let mut pats = Vec::new();
                        while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                            pats.push(self.parse_pat());
                            if !self.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                        self.expect(&TokenKind::RParen, "`)`");
                        Pat::TupleStruct(path, pats, span)
                    }
                    TokenKind::LBrace => {
                        // Struct pattern: Path { field: pat, .. }
                        self.bump();
                        let mut fields = Vec::new();
                        let mut has_rest = false;
                        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                            if *self.peek() == TokenKind::DotDot {
                                self.bump();
                                has_rest = true;
                                break;
                            }
                            let field_ident = self.expect_ident("struct pattern field name");
                            let (field_pat, is_shorthand) = if *self.peek() == TokenKind::Colon {
                                self.bump();
                                (self.parse_pat(), false)
                            } else {
                                // Shorthand: `field` means `field: field`
                                (
                                    Pat::Ident(
                                        BindingMode::ByValue(Mutability::Immutable),
                                        field_ident,
                                        None,
                                    ),
                                    true,
                                )
                            };
                            let f_span = self.current_span();
                            fields.push(PatField {
                                ident: field_ident,
                                pat: field_pat,
                                is_shorthand,
                                span: f_span,
                            });
                            if !self.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                        self.expect(&TokenKind::RBrace, "`}`");
                        Pat::Struct(path, fields, has_rest, span)
                    }
                    _ => {
                        // Lower single-segment no-leading paths (bare identifiers)
                        // to Pat::Ident. This is the common case: `let x = ...`,
                        // `match v { foo => ... }`. Multi-segment paths or paths
                        // with leading (:: / crate:: / etc.) stay as Pat::Path
                        // (e.g., unit-variant patterns like `None`, `Foo::Bar`).
                        if path.segments.len() == 1 && path.leading == PathLeading::None {
                            let ident = path.segments[0].ident;
                            Pat::Ident(BindingMode::ByValue(Mutability::Immutable), ident, None)
                        } else {
                            Pat::Path(path, span)
                        }
                    }
                };
                // `ident @ pat` binding
                if *self.peek() == TokenKind::At {
                    self.bump();
                    let sub_pat = self.parse_pat_no_or();
                    // Extract ident from the path (must be a single-segment path
                    // for `@` binding to be valid).
                    if let Pat::Path(ref p, _) = pat {
                        if p.segments.len() == 1 && p.leading == PathLeading::None {
                            let ident = p.segments[0].ident;
                            return Pat::Ident(
                                BindingMode::ByValue(Mutability::Immutable),
                                ident,
                                Some(Box::new(sub_pat)),
                            );
                        }
                    }
                    // If we already lowered to Pat::Ident, attach the sub-pattern.
                    if let Pat::Ident(mode, ident, None) = pat {
                        return Pat::Ident(mode, ident, Some(Box::new(sub_pat)));
                    }
                    // Fall back: keep the original pat, ignore the @ subpat (with error)
                    self.errors.push(crate::parser::ParseError::new(
                        "`@` binding requires a bare identifier on the left".to_string(),
                        span,
                    ));
                    pat
                } else {
                    pat
                }
            }
            TokenKind::DotDot => {
                // Rest pattern `..` (in slice / struct patterns)
                self.bump();
                Pat::Rest(span)
            }
            _ => {
                // Default: treat as identifier pattern (recovery)
                let ident = self.ident_from_token();
                self.bump();
                Pat::Ident(BindingMode::ByValue(Mutability::Immutable), ident, None)
            }
        }
    }

    pub(super) fn skip_delim_group(&mut self) {
        let (open, close) = match self.peek() {
            TokenKind::LParen => (TokenKind::LParen, TokenKind::RParen),
            TokenKind::LBrace => (TokenKind::LBrace, TokenKind::RBrace),
            TokenKind::LBracket => (TokenKind::LBracket, TokenKind::RBracket),
            _ => return,
        };
        let mut depth = 0;
        while !matches!(self.peek(), TokenKind::Eof) {
            if *self.peek() == open {
                depth += 1;
            } else if *self.peek() == close {
                depth -= 1;
                if depth == 0 {
                    self.bump();
                    return;
                }
            }
            self.bump();
        }
    }
}
