//! Stage 6.12 (TD-022): Generics + bounds + where clause sub-module.
//!
//! Per 02-grammar.md §3.2 (Generic + bound). Extracted from `parser.rs`
//! per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4.
//!
//! Owns:
//! - `parse_params` (function parameter list)
//! - `parse_return_type` (function return type)
//! - `parse_generics` (generic params `<T, 'a, ...>`)
//! - `parse_type_bounds` (trait bounds `Clone + Default + 'static`)
//! - `parse_where_clause` (where clause `where T: Clone`)

use crate::ast::*;
use crate::lexer::token::*;

use super::parser::Parser;

impl<'a> Parser<'a> {
    pub(super) fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
            // self parameter: self, &self, &mut self, mut self, self: Type
            //
            // Stage 15.79 (parser bug fix): the `is_self_param` check
            // previously matched ANY parameter starting with `KwMut`,
            // including regular params like `mut n: i32`. The parser
            // would then consume `n` as if it were the `self` keyword,
            // silently renaming the binding to "self" and producing
            // "cannot find value `n` in this scope" errors for any
            // reference to `n` in the function body.
            //
            // Fix: `mut` alone is NOT a self param — must be `mut self`.
            // The check now verifies that `KwMut` is followed by `KwSelf_`
            // before treating the parameter as a self receiver.
            //
            // Per §1.0 原則 4 "报错 > 静默": the previous behavior silently
            // mis-parsed `mut name: Type` as `mut self: Type`, producing
            // confusing downstream errors instead of correct AST.
            let is_self_param = matches!(self.peek(), TokenKind::KwSelf_)
                || (*self.peek() == TokenKind::KwMut
                    && matches!(self.peek_at(1), TokenKind::KwSelf_))
                || (*self.peek() == TokenKind::And
                    && (matches!(self.peek_at(1), TokenKind::KwSelf_)
                        || (*self.peek_at(1) == TokenKind::KwMut
                            && matches!(self.peek_at(2), TokenKind::KwSelf_))));
            if is_self_param {
                let span = self.current_span();
                // Track the receiver kind: by-value vs by-ref, and mutability.
                let mut self_kind = SelfKind::Value(Mutability::Immutable);
                let binding_mut = if *self.peek() == TokenKind::And {
                    self.bump(); // &
                    let ref_mut = if *self.peek() == TokenKind::KwMut {
                        self.bump();
                        Mutability::Mutable
                    } else {
                        Mutability::Immutable
                    };
                    self_kind = SelfKind::Ref(ref_mut);
                    // For `&self` / `&mut self`, the binding itself is
                    // immutable (you can't reassign `self` even with `&mut self`).
                    Mutability::Immutable
                } else if *self.peek() == TokenKind::KwMut {
                    self.bump();
                    self_kind = SelfKind::Value(Mutability::Mutable);
                    Mutability::Mutable
                } else {
                    // bare `self` — by-value, immutable binding
                    Mutability::Immutable
                };
                self.bump(); // self
                             // self: Type form (rare; usually just `self` or `&self`)
                let ty = if *self.peek() == TokenKind::Colon {
                    self.bump();
                    let parsed_ty = self.parse_ty();
                    // Stage 14.87 (Bug C fix): If the explicit type is a
                    // reference type (&T or &mut T), update self_kind to
                    // match. Was: self_kind stayed as Value(Immutable) even
                    // for `self: &mut Type`, causing mutations to not
                    // propagate to the caller.
                    if let crate::ast::Ty::Ref(_, ref_mut, _, _) = &parsed_ty {
                        self_kind = SelfKind::Ref(*ref_mut);
                    }
                    // For non-ref types (e.g., `self: Type`), leave self_kind
                    // as Value (the default).
                    parsed_ty
                } else {
                    // Default type for self — we use a path with "Self" as the
                    // segment name. The type checker (Stage 2) will resolve this.
                    // Stage 13.17 fix: intern "Self" (capital S) for the type,
                    // matching the resolver's Self type lookup convention.
                    let self_ty_spur = self.interner.get_or_intern("Self");
                    Ty::Path(
                        QSelf::default(),
                        Path {
                            segments: vec![PathSegment {
                                ident: Ident::new(self_ty_spur, span),
                                args: None,
                            }],
                            leading: PathLeading::None,
                            span,
                        },
                        span,
                    )
                };
                // Stage 13.17 fix: intern "self" (lowercase) for the binding
                // name, so the resolver can match `self.x` references in the
                // method body. Previously this used Spur::default() (empty),
                // which never matched the interner's "self" spur — causing
                // "cannot find value in this scope" for ALL self accesses.
                let self_name_spur = self.interner.get_or_intern("self");
                let pat = Pat::Ident(
                    BindingMode::ByValue(binding_mut),
                    Ident::new(self_name_spur, span),
                    None,
                );
                params.push(Param {
                    pat,
                    ty,
                    attrs: Vec::new(),
                    is_self: true,
                    self_kind: Some(self_kind),
                    span,
                });
            } else {
                let pat = self.parse_pat();
                self.expect(&TokenKind::Colon, "`:`");
                let ty = self.parse_ty();
                let span = self.current_span();
                params.push(Param {
                    pat,
                    ty,
                    attrs: Vec::new(),
                    is_self: false,
                    self_kind: None,
                    span,
                });
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        params
    }

    pub(super) fn parse_return_type(&mut self) -> FnRetTy {
        if *self.peek() == TokenKind::Arrow {
            self.bump();
            FnRetTy::Ty(self.parse_ty())
        } else {
            FnRetTy::Default(self.current_span())
        }
    }

    pub(super) fn parse_generics(&mut self) -> Vec<GenericParam> {
        if *self.peek() != TokenKind::Lt {
            return Vec::new();
        }
        self.bump(); // <
        let mut params = Vec::new();
        while !matches!(self.peek(), TokenKind::Gt | TokenKind::Shr | TokenKind::Eof) {
            let span = self.current_span();
            if let TokenKind::Lifetime(_) = self.peek() {
                // Lifetime param: `'a` or `'a: 'b + 'c`
                let ident = self.ident_from_token();
                self.bump();
                let mut bounds = Vec::new();
                if *self.peek() == TokenKind::Colon {
                    self.bump();
                    while let TokenKind::Lifetime(_) = self.peek() {
                        let b_ident = self.ident_from_token();
                        let b_span = self.current_span();
                        self.bump();
                        bounds.push(Lifetime {
                            ident: b_ident,
                            span: b_span,
                        });
                        if !self.eat(&TokenKind::Plus) {
                            break;
                        }
                    }
                }
                params.push(GenericParam::Lifetime(LifetimeParam {
                    ident,
                    bounds,
                    span,
                }));
            } else if matches!(self.peek(), TokenKind::Ident(_) | TokenKind::RawIdent(_)) {
                // Type param: `T` or `T: Bound + Bound` or `T = DefaultTy`
                let ident = self.expect_ident("type parameter name");
                let mut bounds = Vec::new();
                if *self.peek() == TokenKind::Colon {
                    self.bump();
                    bounds = self.parse_type_bounds();
                }
                let default = if *self.peek() == TokenKind::Eq {
                    self.bump();
                    Some(self.parse_ty())
                } else {
                    None
                };
                params.push(GenericParam::Type(TypeParam {
                    ident,
                    bounds,
                    default,
                    span,
                }));
            } else {
                // Unexpected token — recover
                self.errors.push(crate::parser::ParseError::new(
                    format!("expected generic parameter, found {}", self.peek()),
                    span,
                ));
                self.bump();
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        // Handle >> (two >) in type context — split into two >
        if *self.peek() == TokenKind::Shr {
            // We can't actually split a single token; we just consume it and
            // treat it as two closes. Caller logic must be aware.
            // For Stage 0 we just bump it; nested generics like Vec<HashMap<K, V>>
            // will need real >> splitting (Stage 1).
            self.bump();
        } else {
            self.eat(&TokenKind::Gt);
        }
        params
    }

    /// Parse type bounds after `:` e.g. `Clone + Default + 'static`.
    pub(super) fn parse_type_bounds(&mut self) -> Vec<TypeBound> {
        let mut bounds = Vec::new();
        loop {
            let bound = if let TokenKind::Lifetime(_) = self.peek() {
                let ident = self.ident_from_token();
                let span = self.current_span();
                self.bump();
                TypeBound::Lifetime(Lifetime { ident, span })
            } else if matches!(
                self.peek(),
                TokenKind::Ident(_)
                    | TokenKind::RawIdent(_)
                    | TokenKind::PathSep
                    | TokenKind::KwSelf_
                    | TokenKind::KwSelfType
                    | TokenKind::KwCrate
                    | TokenKind::KwSuper
            ) {
                let path = self.parse_path();
                let span = self.current_span();
                TypeBound::Trait(TraitBound {
                    path,
                    args: Vec::new(),
                    span,
                })
            } else {
                break;
            };
            bounds.push(bound);
            if !self.eat(&TokenKind::Plus) {
                break;
            }
        }
        bounds
    }

    pub(super) fn parse_where_clause(&mut self) -> Vec<WherePredicate> {
        if *self.peek() != TokenKind::KwWhere {
            return Vec::new();
        }
        self.bump(); // where
        let mut preds = Vec::new();
        while !matches!(
            self.peek(),
            TokenKind::LBrace | TokenKind::Semicolon | TokenKind::Eof
        ) {
            let span = self.current_span();
            // `'a: 'b + 'c` (lifetime bound) OR `Type: Bound + Bound`
            let lifetime = if let TokenKind::Lifetime(_) = self.peek() {
                let ident = self.ident_from_token();
                let l_span = self.current_span();
                self.bump();
                Some(Lifetime {
                    ident,
                    span: l_span,
                })
            } else {
                None
            };
            let bounded_ty = if lifetime.is_some() {
                // For lifetime predicates, the "type" is a placeholder
                Ty::Tuple(Vec::new(), span)
            } else {
                self.parse_ty()
            };
            self.expect(&TokenKind::Colon, "`:`");
            let bounds = self.parse_type_bounds();
            preds.push(WherePredicate {
                lifetime,
                bounded_ty,
                bounds,
                span,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        preds
    }
}
