//! Stage 6.12 (TD-022): Path parsing sub-module.
//!
//! Per 02-grammar.md §3.1 (path) + §3.3 (type_path + qualified_path).
//! Extracted from `parser.rs` per `docs/stage-committee-process.md` v3.21
//! §14.4 (refactoring as architecture design) and §13.4 (stage-start
//! design alignment with 02-grammar.md §3.1-§3.7).
//!
//! Owns:
//! - `make_path` (path constructor helper)
//! - `parse_path` / `parse_path_in_expr` / `parse_path_in_pat` /
//!   `parse_path_with_ctx` (path parsing in 3 contexts)
//! - `try_parse_turbofish_or_generic_args` / `try_parse_generic_args`
//!   (generic args after path segment)
//!
//! Depends on (from `super`):
//! - `Parser` struct + cursor methods (`peek`/`bump`/`eat`/`expect`/...)
//! - `PathContext` enum (defined in `super`)

use crate::ast::*;
use crate::lexer::token::*;
use crate::session::Span;

// `PathContext` and `Parser` are defined in `parser::parser` (this module's
// parent). From the perspective of `parser::path`, the parent is `parser`,
// so `Parser` is at `super::Parser` (re-exported) and `PathContext` is at
// `super::parser::PathContext`.
use super::parser::{Parser, PathContext};

impl<'a> Parser<'a> {
    #[allow(dead_code)]
    pub(super) fn make_path(
        &self,
        segments: Vec<PathSegment>,
        leading: PathLeading,
        span: Span,
    ) -> Path {
        Path {
            segments,
            leading,
            span,
        }
    }

    pub(super) fn parse_path(&mut self) -> Path {
        self.parse_path_with_ctx(PathContext::Type)
    }

    /// Parse a path in expression position. Differs from `parse_path` in that
    /// generic args are only accepted when introduced by the turbofish `::<>`
    /// syntax — bare `Vec<i32>` in expression position would be ambiguous
    /// with comparison (`a < b`), so we require `Vec::<i32>` instead.
    pub(super) fn parse_path_in_expr(&mut self) -> Path {
        self.parse_path_with_ctx(PathContext::Expr)
    }

    /// Parse a path in pattern position. Like Type, bare `<...>` generic args
    /// are accepted (no ambiguity with comparison in pattern context).
    /// Kept as a separate method to make call-site intent clear and to allow
    /// future divergence (e.g., pattern paths may eventually forbid certain
    /// generic arg forms like assoc type bindings).
    pub(super) fn parse_path_in_pat(&mut self) -> Path {
        self.parse_path_with_ctx(PathContext::Pattern)
    }

    /// The context in which a path is being parsed. Determines whether
    /// bare `<...>` generic args are accepted (Type/Pattern) or whether
    /// turbofish `::<...>` is required (Expr).
    pub(super) fn parse_path_with_ctx(&mut self, ctx: PathContext) -> Path {
        let span = self.current_span();
        let leading = match self.peek() {
            TokenKind::PathSep => {
                self.bump();
                PathLeading::Root
            }
            TokenKind::KwCrate => {
                self.bump();
                PathLeading::Crate
            }
            TokenKind::KwSuper => {
                self.bump();
                PathLeading::Super
            }
            TokenKind::KwSelf_ => {
                self.bump();
                PathLeading::Self_
            }
            _ => PathLeading::None,
        };

        // If we consumed a path-leading keyword (crate/super/self) and the
        // next token is NOT `::`, this is a single-segment path like `self`
        // or `crate` (as a value or module reference).
        if matches!(
            leading,
            PathLeading::Crate | PathLeading::Super | PathLeading::Self_
        ) {
            if *self.peek() != TokenKind::PathSep {
                let kw_str = match leading {
                    PathLeading::Crate => "crate",
                    PathLeading::Super => "super",
                    PathLeading::Self_ => "self",
                    _ => unreachable!(),
                };
                let ident = self.interner.get(kw_str).unwrap_or_default();
                return Path {
                    segments: vec![PathSegment {
                        ident: Ident::new(ident, span),
                        args: None,
                    }],
                    leading: PathLeading::None,
                    span,
                };
            } else {
                // Consume the `::` and continue parsing the rest as a normal path
                self.bump(); // ::
            }
        }

        // After a leading keyword + `::`, OR with no leading keyword, we need
        // an identifier next.
        if !matches!(
            self.peek(),
            TokenKind::Ident(_)
                | TokenKind::RawIdent(_)
                | TokenKind::KwSelf_
                | TokenKind::KwSelfType
                | TokenKind::KwCrate
                | TokenKind::KwSuper
        ) {
            // Not a valid path — return empty path without consuming
            return Path {
                segments: Vec::new(),
                leading,
                span,
            };
        }

        let mut segments = Vec::new();
        let ident = self.ident_from_token();
        self.bump();
        let args = match ctx {
            PathContext::Type | PathContext::Pattern => self.try_parse_generic_args(),
            PathContext::Expr => self.try_parse_turbofish_or_generic_args(),
        };
        segments.push(PathSegment { ident, args });

        while *self.peek() == TokenKind::PathSep {
            // Don't consume `::` if the next token is `{` (use group) or `*`
            // (use glob) — those are handled by parse_use_tree.
            if matches!(self.peek_at(1), TokenKind::LBrace | TokenKind::Star) {
                break;
            }
            self.bump();
            let ident = self.ident_from_token();
            self.bump();
            let args = match ctx {
                PathContext::Type | PathContext::Pattern => self.try_parse_generic_args(),
                PathContext::Expr => self.try_parse_turbofish_or_generic_args(),
            };
            segments.push(PathSegment { ident, args });
        }

        Path {
            segments,
            leading,
            span: Span::new(span.lo, self.current_span().hi),
        }
    }

    /// Parse generic arguments `<...>` after a path segment, if present.
    /// Returns None if no `<` is present.
    ///
    /// Per 02-grammar.md §3.3: `Vec<i32>`, `HashMap<K, V>`, `Foo<'a, T>`,
    /// `Iterator<Item = i32>` (assoc type).
    ///
    /// Disambiguation: `<` could be a comparison operator (`a < b`). We use a
    /// heuristic: only treat as generic args if the next token after `<` is
    /// an identifier, raw identifier, lifetime, `>` (empty generics, rare),
    /// `?` (Sized/`?Sized`), or a type keyword. If it's a numeric literal,
    /// string literal, or anything else, we treat `<` as comparison.
    /// In expression position, generic args must be introduced with the
    /// turbofish syntax `::<...>`. This method peeks for `::` `<` and, if
    /// present, consumes the `::` and delegates to `try_parse_generic_args`.
    /// Returns `None` if no turbofish is present (the `<` if any is left for
    /// the caller to interpret as a comparison operator).
    pub(super) fn try_parse_turbofish_or_generic_args(&mut self) -> Option<GenericArgs> {
        if *self.peek() == TokenKind::PathSep && *self.peek_at(1) == TokenKind::Lt {
            self.bump(); // ::
            return self.try_parse_generic_args();
        }
        None
    }

    /// Parse generic arguments `<...>` after a path segment, if present.
    /// Returns None if no `<` is present.
    ///
    /// Per 02-grammar.md §3.3: `Vec<i32>`, `HashMap<K, V>`, `Foo<'a, T>`,
    /// `Iterator<Item = i32>` (assoc type).
    ///
    /// Disambiguation: `<` could be a comparison operator (`a < b`). We use a
    /// heuristic: only treat as generic args if the next token after `<` is
    /// an identifier, raw identifier, lifetime, `>` (empty generics, rare),
    /// `?` (Sized/`?Sized`), or a type keyword. If it's a numeric literal,
    /// string literal, or anything else, we treat `<` as comparison.
    pub(super) fn try_parse_generic_args(&mut self) -> Option<GenericArgs> {
        if *self.peek() != TokenKind::Lt {
            return None;
        }
        // Lookahead: only proceed if this looks like generic args, not comparison.
        let next = self.peek_at(1);
        let looks_like_generic = matches!(
            next,
            TokenKind::Ident(_) | TokenKind::RawIdent(_)
            | TokenKind::Lifetime(_)
            | TokenKind::KwSelf_ | TokenKind::KwSelfType
            | TokenKind::KwCrate | TokenKind::KwSuper
            | TokenKind::PathSep  // ::Path
            | TokenKind::LParen   // (T, U) tuple type arg
            | TokenKind::LBracket // [T] array/slice type arg
            | TokenKind::And      // &T reference type arg
            | TokenKind::Star     // *const T / *mut T pointer type arg
            | TokenKind::Not      // ! never type arg
            | TokenKind::KwFn     // fn(...) -> T pointer type arg
            | TokenKind::KwImpl   // impl Trait (rare in args)
            | TokenKind::KwDyn    // dyn Trait
            | TokenKind::Gt       // `<>` (rare, e.g. `PhantomData<>`)
            | TokenKind::Shr // `>>`
        );
        if !looks_like_generic {
            return None;
        }
        self.bump(); // <
        let mut args = Vec::new();
        while !matches!(self.peek(), TokenKind::Gt | TokenKind::Shr | TokenKind::Eof) {
            if let TokenKind::Lifetime(_) = self.peek() {
                let ident = self.ident_from_token();
                let l_span = self.current_span();
                self.bump();
                args.push(GenericArg::Lifetime(Lifetime {
                    ident,
                    span: l_span,
                }));
            } else if matches!(self.peek(), TokenKind::Ident(_) | TokenKind::RawIdent(_))
                && *self.peek_at(1) == TokenKind::Eq
            {
                // Associated type binding: `Item = i32`
                let name = self.expect_ident("associated type binding name");
                self.bump(); // =
                let ty = self.parse_ty();
                args.push(GenericArg::Assoc(name, ty));
            } else {
                // Type argument
                let ty = self.parse_ty();
                args.push(GenericArg::Type(ty));
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        // Handle >> (nested generics) — just consume for Stage 0
        if *self.peek() == TokenKind::Shr {
            self.bump();
        } else {
            self.eat(&TokenKind::Gt);
        }
        Some(GenericArgs::AngleBracketed(args))
    }
}
