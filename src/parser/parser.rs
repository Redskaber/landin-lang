//! Recursive descent + Pratt parser.
//!
//! Based on 02-grammar.md §2-3.
//!
//! ## Stage 6.12 architectural split (TD-022)
//!
//! Per `docs/stage-committee-process.md` v3.21 §14.4 (refactoring as
//! architecture design) and §13.4 (stage-start design alignment with
//! 02-grammar.md §3.1-§3.7), this file has been split into 7 sub-modules:
//!
//! - `path.rs`     — path parsing (§3.1 path + PathContext)
//! - `generics.rs` — generic + bound + where + params + return type (§3.2)
//! - `ty.rs`       — type parsing (§3.3)
//! - `expr.rs`     — expression Pratt parsing (§3.4)
//! - `pat.rs`      — pattern parsing (§3.5)
//! - `stmt.rs`     — block + let statement (§3.6)
//! - `items.rs`    — item-level parsing: fn/struct/enum/trait/impl/... (§3.1 + §3.7)
//!
//! This file (`parser.rs`) retains: Parser struct + cursor methods +
//! `parse_crate` entry point + `recover` error-recovery.
//!
//! All sub-modules add methods to `impl<'a> Parser<'a>` via their own
//! `impl` blocks. Cursor methods are `pub(super)` so sibling modules
//! can call them. Per §16, parser-external code only sees `parse_crate`.

use crate::ast::*;
use crate::lexer::token::*;
use crate::session::Span;
use lasso::{Rodeo, Spur};

// Sub-modules (Stage 6.12 split, per §14.4 + §13.4).
// Declared in `src/parser/mod.rs` so they live in `src/parser/` (sibling to
// this file). See `mod.rs` for the `mod expr; mod generics; ...` declarations.

/// Context in which a path is being parsed. Determines whether bare `<...>`
/// generic args are accepted (Type/Pattern) or whether turbofish `::<...>`
/// is required (Expr).
///
/// Rationale: in expression position, `a < b` is a comparison, not
/// `a::<b>` (generic args on a value-path). To get generic args in expr
/// position, the user must write the turbofish form `a::<b>`. In type
/// position there is no ambiguity — `<` is always generic args.
///
/// Stage 6.12: this enum is `pub(super)` so the `path` sub-module can use it
/// (it lives in `path.rs`'s impl block).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PathContext {
    Type,
    Expr,
    Pattern,
}

pub struct Parser<'a> {
    pub(super) tokens: Vec<Token>,
    pub(super) pos: usize,
    /// Stage 3.30: changed from `&'a Rodeo` to `&'a mut Rodeo` so the
    /// parser can intern tuple-field indices (`p.0`, `p.1`) — was using
    /// `Spur::default()` which lost the index. Per §15, this is a root-cause
    /// fix (not a workaround).
    pub(super) interner: &'a mut Rodeo,
    pub(super) errors: Vec<crate::parser::ParseError>,
    /// When true, `parse_primary_expr` will NOT try to parse a `{` following
    /// a path as a struct literal. This is set to `true` while parsing the
    /// condition of `if` / `while` / `for` / `match` so that the `{` belongs
    /// to the block, not a struct literal in the condition.
    pub(super) no_struct_literal: bool,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token>, interner: &'a mut Rodeo) -> Self {
        Self {
            tokens,
            pos: 0,
            interner,
            errors: Vec::new(),
            no_struct_literal: false,
        }
    }

    pub fn into_errors(self) -> Vec<crate::parser::ParseError> {
        self.errors
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    // --- Token helpers ---

    pub(super) fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    pub(super) fn peek_at(&self, n: usize) -> &TokenKind {
        self.tokens
            .get(self.pos + n)
            .map(|t| &t.kind)
            .unwrap_or(&TokenKind::Eof)
    }

    pub(super) fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or(Span::DUMMY)
    }

    pub(super) fn bump(&mut self) -> &TokenKind {
        let kind = &self.tokens[self.pos].kind;
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        kind
    }

    pub(super) fn eat(&mut self, expected: &TokenKind) -> bool {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(super) fn expect(&mut self, expected: &TokenKind, what: &str) -> Span {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(expected) {
            let span = self.current_span();
            self.bump();
            span
        } else {
            let span = self.current_span();
            self.errors.push(crate::parser::ParseError::new(
                format!("expected {what}, found {}", self.peek()),
                span,
            ));
            span
        }
    }

    /// Expect an identifier (either `Ident` or `RawIdent`).
    ///
    /// Per 02-grammar.md §1.2, raw identifiers are accepted in all name
    /// positions (function names, struct field names, let-binding names, etc.).
    /// Returns the consumed `Ident` (with span) and advances past the token.
    pub(super) fn expect_ident(&mut self, what: &str) -> Ident {
        let span = self.current_span();
        match self.peek() {
            TokenKind::Ident(_) | TokenKind::RawIdent(_) => {
                let ident = self.ident_from_token();
                self.bump();
                ident
            }
            _ => {
                self.errors.push(crate::parser::ParseError::new(
                    format!("expected {what}, found {}", self.peek()),
                    span,
                ));
                Ident::new(Spur::default(), span)
            }
        }
    }

    pub(super) fn ident_from_token(&self) -> Ident {
        // Convert any identifier-like token (Ident, RawIdent, or the path
        // keyword segments self/Self/crate/super) to an Ident. For keywords,
        // we intern the keyword string into the interner so the symbol is
        // stable. The interner is borrowed immutably here so we use
        // `get` (lookup-only); if the symbol isn't present yet, we fall back
        // to Spur::default() — this is acceptable for Stage 0 because Stage 1
        // name resolution will re-intern keyword segments with the proper
        // canonical symbol via a mutable interner pass.
        match &self.tokens[self.pos].kind {
            TokenKind::Ident(sym) | TokenKind::RawIdent(sym) => {
                Ident::new(*sym, self.current_span())
            }
            TokenKind::KwSelf_ => {
                // Try to find an already-interned "self" symbol; otherwise default.
                self.interner
                    .get("self")
                    .map(|s| Ident::new(s, self.current_span()))
                    .unwrap_or_else(|| Ident::new(Spur::default(), self.current_span()))
            }
            TokenKind::KwSelfType => self
                .interner
                .get("Self")
                .map(|s| Ident::new(s, self.current_span()))
                .unwrap_or_else(|| Ident::new(Spur::default(), self.current_span())),
            TokenKind::KwCrate => self
                .interner
                .get("crate")
                .map(|s| Ident::new(s, self.current_span()))
                .unwrap_or_else(|| Ident::new(Spur::default(), self.current_span())),
            TokenKind::KwSuper => self
                .interner
                .get("super")
                .map(|s| Ident::new(s, self.current_span()))
                .unwrap_or_else(|| Ident::new(Spur::default(), self.current_span())),
            _ => Ident::new(Spur::default(), self.current_span()),
        }
    }

    // --- Entry point ---

    pub fn parse_crate(&mut self) -> Crate {
        let mut items = Vec::new();
        while *self.peek() != TokenKind::Eof {
            // Skip doc comments at item position.
            //
            // Per 05-ast.md §10, doc comments should attach to the next item
            // as attributes. Proper attachment requires the attribute system
            // (Stage 1, Month 3). For Stage 0 we accept and discard them so
            // that the presence of doc comments doesn't break parsing.
            //
            // Inner doc comments (`//!`) at crate root are likewise skipped.
            if matches!(self.peek(), TokenKind::DocComment(_, _)) {
                self.bump();
                continue;
            }
            match self.parse_item() {
                Some(item) => items.push(item),
                None => {
                    // Error recovery: skip to next sync token
                    self.recover();
                }
            }
        }
        Crate {
            items,
            attrs: Vec::new(),
        }
    }

    // --- Error recovery ---

    pub(super) fn recover(&mut self) {
        // Skip to next sync token, but ALWAYS advance to prevent infinite loops
        while !matches!(
            self.peek(),
            TokenKind::Eof
                | TokenKind::Semicolon
                | TokenKind::RBrace
                | TokenKind::KwFn
                | TokenKind::KwStruct
                | TokenKind::KwEnum
                | TokenKind::KwImpl
                | TokenKind::KwTrait
                | TokenKind::KwPub
                | TokenKind::KwUse
                | TokenKind::KwMod
                | TokenKind::KwConst
                | TokenKind::KwStatic
                | TokenKind::KwType
                | TokenKind::KwExtern
        ) {
            self.bump();
        }
        // ALWAYS bump the sync token to prevent infinite loop on RBrace
        // (RBrace might close an outer block, so we eat it to make progress)
        if !matches!(self.peek(), TokenKind::Eof) {
            self.bump();
        }
    }
}
