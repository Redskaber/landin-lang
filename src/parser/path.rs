//! Stage 6.12 (TD-022): Path parsing sub-module.
//!
//! Per 02-grammar.md §3.1 (path) + §3.3 (type_path + qualified_path).
//! Extracted from `parser.rs` per `docs/stage-committee-process.md` v3.21
//! §14.4 (refactoring as architecture design) and §13.4 (stage-start
//! design alignment with 02-grammar.md §3.1-§3.7).
//!
//! Owns:
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

        // Stage 18.53 GATs Phase 2: Qualified path `<T as Trait>::Name`.
        // Per §1.0 原則 6 "通用 > 特例": one try_parse_qself handles all
        // qualified path forms. Only attempted in Type / Pattern context —
        // in Expr context, `<` is comparison.
        //
        // Stage 18.55 GATs Phase 3: Also handle `<<` (Shl) when inside a
        // `<<` split. When `shl_split > 0`, the current token is `<<` but
        // we've already "consumed" one `<` — the next `eat_lt_or_split`
        // will consume the other. So `try_parse_qself` should be attempted
        // when peek is `<` OR when peek is `<<` with `shl_split > 0`.
        //
        // Note: qself info (inner type + position) is stored on the wrapping
        // `Ty::Path(QSelf, Path, Span)`, NOT on `Path` itself. When called
        // from `parse_ty`, the caller checks for qself and wraps accordingly.
        // When called from non-type contexts (Expr, Pattern), qself info is
        // discarded (qualified paths in expression position require different
        // handling — turbofish + UFCS — which is out of scope for Phase 2).
        let is_qself_start = matches!(ctx, PathContext::Type | PathContext::Pattern)
            && (*self.peek() == TokenKind::Lt
                || (*self.peek() == TokenKind::Shl && self.shl_split > 0));
        if is_qself_start {
            if let Some((_qself, path)) = self.try_parse_qself(ctx, span) {
                // QSelf info is preserved via thread-local storage for
                // parse_ty to pick up. See `take_last_qself` below.
                // (Stage 18.53: simple approach — store in a field on Parser.)
                self.last_qself = Some(_qself);
                return path;
            }
            // If qself parse failed (no `as` keyword etc.), fall through to
            // normal path parsing — the caller will see `<` and produce an
            // error or treat as comparison depending on context.
        }

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
                    // Guarded by `matches!` above: only Crate|Super|Self_ reach here.
                    _ => unreachable!("matches! guard ensures only Crate|Super|Self_"),
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
        // Stage 18.55: Accept both `<` and `<<` (Shl) as generic args start.
        // `<<` occurs in nested qualified paths like `Vec<<T as Trait>::Item>`.
        if !matches!(self.peek(), TokenKind::Lt | TokenKind::Shl) {
            return None;
        }
        // Lookahead: only proceed if this looks like generic args, not comparison.
        // When the current token is `<<`, peek_at(1) is the token after `<<`,
        // which should be the start of the inner type (e.g., `T` in `<<T as C>::Item>`).
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
            | TokenKind::Lt // `<<T as Trait>::Item>` (nested qualified path)
            | TokenKind::Shl // `<<T as Trait>::Item>` (Stage 18.55: `<<` splitting)
        );
        if !looks_like_generic {
            return None;
        }
        // Stage 18.55: Use `eat_lt_or_split` to handle `<<` in nested generics
        // like `Vec<<T as Trait>::Item>`. Per §1.0 原則 6 "通用 > 特例".
        if !self.eat_lt_or_split() {
            return None; // shouldn't happen (lookahead already verified `<` or `<<`)
        }
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
        // Stage 18.53 GATs Phase 2: Use `eat_gt_or_split` to handle `>>`
        // in nested generics like `Vec<HashMap<K, V>>` or
        // `Option<Self::Item<'a>>`. Per §1.0 原則 6 "通用 > 特例".
        //
        // Note: We do NOT report an error if `>` is missing — the original
        // behavior was to silently `eat(Gt)` (no-op on failure), which lets
        // the parser recover from cases like `let y: x<x;` where `<x` was
        // ambiguously parsed as generic args. Reporting an error here would
        // break 80+ conformance tests that rely on this graceful recovery.
        // The GAT-specific error cases are caught by `try_parse_qself` which
        // has its own error reporting for missing `>`.
        if !self.eat_gt_or_split() {
            self.eat(&TokenKind::Gt);
        }
        Some(GenericArgs::AngleBracketed(args))
    }

    /// Stage 30.9 (v0.14 TD-HRTB-FN-SYNTAX): Parse parenthesized generic args
    /// `Fn(T1, T2) -> U` — the function-trait syntax.
    ///
    /// This is used by `Fn`, `FnMut`, `FnOnce` trait bounds:
    /// `F: Fn(i32) -> i32` or `for<'a> Fn(&'a T) -> &'a U`.
    ///
    /// Returns `Some(GenericArgs::Parenthesized(inputs, output))` if the
    /// next token is `(`, otherwise `None`.
    ///
    /// Per §1.0 原則 3 (显式 > 隐式): the parenthesized form is explicit.
    /// Per §1.0 原則 6 (通解 > 特解): one parser for all Fn/FnMut/FnOnce.
    /// Per §23: function name follows `<verb>_<noun>_<noun>` pattern.
    pub(super) fn try_parse_parenthesized_args(&mut self) -> Option<GenericArgs> {
        if *self.peek() != TokenKind::LParen {
            return None;
        }
        self.bump(); // (
        let mut inputs = Vec::new();
        while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
            inputs.push(self.parse_ty());
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen, "`)`");
        let output = if *self.peek() == TokenKind::Arrow {
            self.bump(); // ->
            self.parse_ty()
        } else {
            // No return type → unit `()`
            Ty::Tuple(Vec::new(), self.current_span())
        };
        Some(GenericArgs::Parenthesized(inputs, Box::new(output)))
    }

    /// Stage 18.53 GATs Phase 2: Parse a qualified path `<T as Trait>::Name`.
    ///
    /// Returns `Some((QSelf, Path))` on success, where `QSelf.ty = Some(T)`
    /// and `QSelf.position` is the number of trait segments. Returns `None`
    /// if the input doesn't look like a qualified path (e.g., no `as`
    /// keyword after the inner type).
    ///
    /// ## Grammar
    ///
    /// ```text
    /// qualified_path := "<" ty "as" path ">" "::" path_segment ("::" path_segment)*
    /// ```
    ///
    /// ## Algorithm
    ///
    /// 1. Consume `<`
    /// 2. Parse inner type `T`
    /// 3. Expect `as` keyword (if missing, return None — caller falls back)
    /// 4. Parse trait path (segments until `>`)
    /// 5. Consume `>` (using `eat_gt_or_split` to support nested generics)
    /// 6. Expect `::`
    /// 7. Parse remaining segments (the assoc item and any further path)
    ///
    /// Per §1.0 原則 6 "通用 > 特例": one parser handles all qualified path
    /// forms, no per-use-site special cases.
    /// Per §10 naming: `try_parse_qself` follows `<verb>_<noun>` pattern.
    pub(super) fn try_parse_qself(
        &mut self,
        ctx: PathContext,
        span: crate::session::Span,
    ) -> Option<(QSelf, Path)> {
        // Stage 18.55: Accept both `<` and `<<` (with shl_split > 0) as qself start.
        // When `shl_split > 0`, the current token is `<<` but we've already
        // "consumed" one `<` — `eat_lt_or_split` will consume the other.
        if !matches!(self.peek(), TokenKind::Lt | TokenKind::Shl) {
            return None;
        }
        // If it's `<<` but shl_split == 0, this is a fresh `<<` — we should
        // NOT attempt qself here (the caller `try_parse_generic_args` handles
        // the initial `<<` split; qself is only attempted after the first `<`
        // is consumed). So only proceed if `<` or `<<` with split > 0.
        if *self.peek() == TokenKind::Shl && self.shl_split == 0 {
            return None;
        }

        // Save position for rollback if this isn't actually a qself.
        let saved_pos = self.pos;
        let saved_errors_len = self.errors.len();
        let saved_shl_split = self.shl_split;

        // Stage 18.55: Use `eat_lt_or_split` to handle `<<` (when shl_split > 0,
        // this consumes the second `<` of a `<<` split; otherwise it consumes
        // a plain `<`).
        self.eat_lt_or_split();

        // Parse the inner type T.
        let inner_ty = self.parse_ty();

        // Expect `as` keyword. If not present, this isn't a qself — rollback.
        if *self.peek() != TokenKind::KwAs {
            self.pos = saved_pos;
            self.errors.truncate(saved_errors_len);
            self.shl_split = saved_shl_split;
            return None;
        }
        self.bump(); // as

        // Parse the trait path segments until `>`.
        let mut segments: Vec<PathSegment> = Vec::new();
        // The first segment must be an identifier.
        // Per §1.0 原則 4 "报错 > 静默": use `expect_ident` instead of
        // `ident_from_token` so non-identifier tokens (e.g., `@`) produce
        // a parse error instead of silently becoming default Idents.
        let ident = self.expect_ident("trait name in qualified path");
        let args = match ctx {
            PathContext::Type | PathContext::Pattern => self.try_parse_generic_args(),
            PathContext::Expr => self.try_parse_turbofish_or_generic_args(),
        };
        segments.push(PathSegment { ident, args });

        // Continue trait path: `Trait::SubTrait::...`
        while *self.peek() == TokenKind::PathSep {
            // Stop if next is `>` (end of qself) — `::>` is invalid.
            if matches!(self.peek_at(1), TokenKind::Gt | TokenKind::Shr) {
                break;
            }
            self.bump(); // ::
            let ident = self.expect_ident("trait name in qualified path");
            let args = match ctx {
                PathContext::Type | PathContext::Pattern => self.try_parse_generic_args(),
                PathContext::Expr => self.try_parse_turbofish_or_generic_args(),
            };
            segments.push(PathSegment { ident, args });
        }

        // Record where the trait path ends (qself.position).
        let trait_position = segments.len();

        // Consume `>` (with `>>` split support for nested generics).
        if !self.eat_gt_or_split() {
            // Missing `>` — record parse error but continue to extract what we can.
            self.errors.push(crate::parser::ParseError::new(
                format!("expected `>` in qualified path, found {}", self.peek()),
                self.current_span(),
            ));
        }

        // Expect `::` after `>`.
        if *self.peek() != TokenKind::PathSep {
            self.errors.push(crate::parser::ParseError::new(
                format!(
                    "expected `::` after `>` in qualified path, found {}",
                    self.peek()
                ),
                self.current_span(),
            ));
            // Build a path with just the trait segments — best-effort recovery.
            return Some((
                QSelf {
                    ty: Some(Box::new(inner_ty)),
                    position: trait_position,
                },
                Path {
                    segments,
                    leading: PathLeading::None,
                    span: Span::new(span.lo, self.current_span().hi),
                },
            ));
        }
        self.bump(); // ::

        // Parse the remaining segments (the assoc item name and any further path).
        let ident = self.expect_ident("associated item name in qualified path");
        let args = match ctx {
            PathContext::Type | PathContext::Pattern => self.try_parse_generic_args(),
            PathContext::Expr => self.try_parse_turbofish_or_generic_args(),
        };
        segments.push(PathSegment { ident, args });

        while *self.peek() == TokenKind::PathSep {
            if matches!(self.peek_at(1), TokenKind::LBrace | TokenKind::Star) {
                break;
            }
            self.bump();
            let ident = self.expect_ident("path segment in qualified path");
            let args = match ctx {
                PathContext::Type | PathContext::Pattern => self.try_parse_generic_args(),
                PathContext::Expr => self.try_parse_turbofish_or_generic_args(),
            };
            segments.push(PathSegment { ident, args });
        }

        Some((
            QSelf {
                ty: Some(Box::new(inner_ty)),
                position: trait_position,
            },
            Path {
                segments,
                leading: PathLeading::None,
                span: Span::new(span.lo, self.current_span().hi),
            },
        ))
    }
}
