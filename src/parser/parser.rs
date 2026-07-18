//! Recursive descent + Pratt parser.
//!
//! Based on 02-grammar.md §2-3.

use crate::ast;
use crate::ast::*;
use crate::lexer::token::*;
use crate::session::Span;
use lasso::{Rodeo, Spur};

pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    interner: &'a Rodeo,
    errors: Vec<crate::parser::ParseError>,
    /// When true, `parse_primary_expr` will NOT try to parse a `{` following
    /// a path as a struct literal. This is set to `true` while parsing the
    /// condition of `if` / `while` / `for` / `match` so that the `{` belongs
    /// to the block, not a struct literal in the condition.
    no_struct_literal: bool,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token>, interner: &'a Rodeo) -> Self {
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

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_at(&self, n: usize) -> &TokenKind {
        self.tokens
            .get(self.pos + n)
            .map(|t| &t.kind)
            .unwrap_or(&TokenKind::Eof)
    }

    fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or(Span::DUMMY)
    }

    fn bump(&mut self) -> &TokenKind {
        let kind = &self.tokens[self.pos].kind;
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        kind
    }

    fn eat(&mut self, expected: &TokenKind) -> bool {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: &TokenKind, what: &str) -> Span {
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
    fn expect_ident(&mut self, what: &str) -> Ident {
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

    fn ident_from_token(&self) -> Ident {
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

    #[allow(dead_code)]
    fn make_path(&self, segments: Vec<PathSegment>, leading: PathLeading, span: Span) -> Path {
        Path {
            segments,
            leading,
            span,
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

    fn recover(&mut self) {
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

    // --- Items ---

    fn parse_item(&mut self) -> Option<Item> {
        let span = self.current_span();
        // Standard Rust convention: attributes can come before OR after visibility.
        // We parse them in a loop accepting either order.
        let mut attrs = Vec::new();
        let mut vis = Visibility::Private;
        loop {
            if *self.peek() == TokenKind::Hash && matches!(self.peek_at(1), TokenKind::LBracket) {
                attrs.extend(self.parse_outer_attrs());
            } else if *self.peek() == TokenKind::KwPub {
                vis = self.parse_visibility();
            } else {
                break;
            }
        }

        let kind = match self.peek() {
            TokenKind::KwFn => ItemKind::Fn(self.parse_fn(false, Abi::Landin)),
            TokenKind::KwConst => ItemKind::Const(self.parse_const()),
            TokenKind::KwStatic => ItemKind::Static(self.parse_static()),
            TokenKind::KwStruct => ItemKind::Struct(self.parse_struct()),
            TokenKind::KwEnum => ItemKind::Enum(self.parse_enum()),
            TokenKind::KwTrait => ItemKind::Trait(self.parse_trait()),
            TokenKind::KwImpl => ItemKind::Impl(self.parse_impl()),
            TokenKind::KwType => ItemKind::TypeAlias(self.parse_type_alias()),
            TokenKind::KwExtern => ItemKind::ExternBlock(self.parse_extern_block_or_fn()),
            TokenKind::KwMod => ItemKind::Mod(self.parse_mod()),
            TokenKind::KwUse => ItemKind::Use(self.parse_use()),
            // unsafe fn — `unsafe` keyword followed by `fn`
            TokenKind::KwUnsafe if matches!(self.peek_at(1), TokenKind::KwFn) => {
                self.bump(); // consume `unsafe`
                ItemKind::Fn(self.parse_fn(true, Abi::Landin))
            }
            // unsafe impl — `unsafe impl Trait for T {}`
            // The AST doesn't have an `is_unsafe` field on ImplDecl yet (Stage 1.0
            // work); for Stage 0 we accept and parse the impl, dropping the
            // `unsafe` qualifier. Stage 1 will extend the AST.
            TokenKind::KwUnsafe if matches!(self.peek_at(1), TokenKind::KwImpl) => {
                self.bump(); // consume `unsafe`
                ItemKind::Impl(self.parse_impl())
            }
            // unsafe trait — `unsafe trait Foo { ... }`
            // Same caveat: AST lacks is_unsafe on TraitDecl; Stage 1 will extend.
            TokenKind::KwUnsafe if matches!(self.peek_at(1), TokenKind::KwTrait) => {
                self.bump(); // consume `unsafe`
                ItemKind::Trait(self.parse_trait())
            }
            _ => {
                self.errors.push(crate::parser::ParseError::new(
                    format!("expected item, found {}", self.peek()),
                    span,
                ));
                return None;
            }
        };

        Some(Item {
            vis,
            attrs,
            kind,
            span,
        })
    }

    /// Parse zero or more outer attributes `#[...]` preceding an item.
    ///
    /// Per 02-grammar.md §3.1 + 15-attributes.md: an attribute is `#` `[` path
    /// attr-args? `]`. Inner attributes `#![...]` are handled at crate level
    /// (Stage 1); for Stage 0 we only parse outer attributes here.
    ///
    /// Attribute args can be: empty `#[foo]`, literal `#[foo = "lit"]`,
    /// list `#[foo(a, b, c)]`, or eq-expr `#[foo = expr]`.
    fn parse_outer_attrs(&mut self) -> Vec<Attr> {
        let mut attrs = Vec::new();
        while *self.peek() == TokenKind::Hash {
            // Must be followed by `[` for outer attr, or `![` for inner attr.
            // We only handle outer here.
            if !matches!(self.peek_at(1), TokenKind::LBracket) {
                break;
            }
            let attr_span = self.current_span();
            self.bump(); // #
            self.bump(); // [
            let path = self.parse_path();
            let args = if *self.peek() == TokenKind::RBracket {
                None
            } else {
                Some(self.parse_attr_args())
            };
            self.expect(&TokenKind::RBracket, "`]`");
            attrs.push(Attr {
                path,
                args,
                span: attr_span,
            });
        }
        attrs
    }

    /// Parse attribute arguments after the path.
    fn parse_attr_args(&mut self) -> AttrArgs {
        match self.peek() {
            TokenKind::Eq => {
                self.bump();
                let expr = self.parse_expr();
                AttrArgs::Eq(expr)
            }
            TokenKind::LParen => {
                self.bump();
                let mut args = Vec::new();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    let name =
                        if matches!(self.peek(), TokenKind::Ident(_) | TokenKind::RawIdent(_)) {
                            Some(self.expect_ident("attribute argument name"))
                        } else {
                            None
                        };
                    let value = if *self.peek() == TokenKind::Eq {
                        self.bump();
                        Some(self.parse_expr())
                    } else if name.is_none() {
                        // Bare expression argument, e.g. `derive(Debug, Clone)`
                        Some(self.parse_expr())
                    } else {
                        None
                    };
                    args.push(AttrArg { name, value });
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen, "`)`");
                AttrArgs::List(args)
            }
            // Literal: `#[foo = "lit"]` already handled by Eq arm; this is for
            // unusual forms like `#[foo "lit"]` (rare but valid).
            TokenKind::StrLit(_) | TokenKind::IntLit(_, _) | TokenKind::FloatLit(_, _) => {
                let lit_kind = self.parse_primary_expr();
                if let Expr::Lit(kind, _) = lit_kind {
                    AttrArgs::Literal(kind)
                } else {
                    AttrArgs::Empty
                }
            }
            _ => AttrArgs::Empty,
        }
    }

    fn parse_visibility(&mut self) -> Visibility {
        if *self.peek() == TokenKind::KwPub {
            self.bump();
            // pub(crate) / pub(super) / pub(self) / pub(in path)
            if *self.peek() == TokenKind::LParen {
                self.bump();
                // `pub(in path)` — the `in` keyword precedes the path
                if *self.peek() == TokenKind::KwIn {
                    self.bump();
                }
                let path = self.parse_path();
                self.expect(&TokenKind::RParen, "`)`");
                return Visibility::PubRestricted(path);
            }
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    fn parse_fn(&mut self, is_unsafe: bool, abi: Abi) -> FnDecl {
        self.bump(); // fn
        let name = self.expect_ident("function name");
        let generics = self.parse_generics();
        self.expect(&TokenKind::LParen, "`(`");
        let inputs = self.parse_params();
        self.expect(&TokenKind::RParen, "`)`");
        let output = self.parse_return_type();
        let where_clause = self.parse_where_clause();
        let sig_span = self.current_span();
        let generics = Generics {
            params: generics,
            where_clause,
            span: self.current_span(),
        };

        let body = if *self.peek() == TokenKind::LBrace {
            Some(self.parse_block())
        } else {
            self.expect(&TokenKind::Semicolon, "`{` or `;`");
            None
        };

        FnDecl {
            ident: name,
            sig: FnSig {
                inputs,
                output,
                abi,
                is_unsafe,
                span: sig_span,
            },
            body,
            generics,
        }
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
            // self parameter: self, &self, &mut self, mut self, self: Type
            let is_self_param = matches!(self.peek(), TokenKind::KwSelf_ | TokenKind::KwMut)
                || (*self.peek() == TokenKind::And
                    && matches!(self.peek_at(1), TokenKind::KwSelf_ | TokenKind::KwMut));
            if is_self_param {
                let span = self.current_span();
                // Handle &self / &mut self
                if *self.peek() == TokenKind::And {
                    self.bump(); // &
                    if *self.peek() == TokenKind::KwMut {
                        self.bump();
                    }
                } else if *self.peek() == TokenKind::KwMut {
                    self.bump();
                }
                self.bump(); // self
                             // self: Type form (rare; usually just `self` or `&self`)
                let ty = if *self.peek() == TokenKind::Colon {
                    self.bump();
                    self.parse_ty()
                } else {
                    // Default type for self — we use a path with KwSelf_ as the
                    // segment name. The type checker (Stage 2) will resolve this.
                    Ty::Path(
                        QSelf::default(),
                        Path {
                            segments: vec![PathSegment {
                                ident: Ident::new(Spur::default(), span),
                                args: None,
                            }],
                            leading: PathLeading::None,
                            span,
                        },
                        span,
                    )
                };
                let pat = Pat::Ident(
                    BindingMode::ByValue,
                    Ident::new(Spur::default(), span),
                    None,
                );
                params.push(Param {
                    pat,
                    ty,
                    attrs: Vec::new(),
                    is_self: true,
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
                    span,
                });
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        params
    }

    fn parse_return_type(&mut self) -> FnRetTy {
        if *self.peek() == TokenKind::Arrow {
            self.bump();
            FnRetTy::Ty(self.parse_ty())
        } else {
            FnRetTy::Default(self.current_span())
        }
    }

    fn parse_generics(&mut self) -> Vec<GenericParam> {
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
    fn parse_type_bounds(&mut self) -> Vec<TypeBound> {
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

    fn parse_where_clause(&mut self) -> Vec<WherePredicate> {
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

    fn parse_const(&mut self) -> ConstDecl {
        self.bump(); // const
        let ident = self.ident_from_token();
        self.bump(); // ident
        self.expect(&TokenKind::Colon, "`:`");
        let ty = self.parse_ty();
        self.expect(&TokenKind::Eq, "`=`");
        let expr = self.parse_expr();
        self.expect(&TokenKind::Semicolon, "`;`");
        ConstDecl {
            ident,
            ty,
            expr,
            is_const: true,
            is_mut: false,
            span: Span::DUMMY,
        }
    }

    fn parse_static(&mut self) -> StaticDecl {
        self.bump(); // static
        let is_mut = self.eat(&TokenKind::KwMut);
        let ident = self.ident_from_token();
        self.bump(); // ident
        self.expect(&TokenKind::Colon, "`:`");
        let ty = self.parse_ty();
        self.expect(&TokenKind::Eq, "`=`");
        let expr = self.parse_expr();
        self.expect(&TokenKind::Semicolon, "`;`");
        ConstDecl {
            ident,
            ty,
            expr,
            is_const: false,
            is_mut,
            span: Span::DUMMY,
        }
    }

    fn parse_struct(&mut self) -> StructDecl {
        self.bump(); // struct
        let ident = self.ident_from_token();
        self.bump(); // ident
        let generics = self.parse_generics();
        let where_clause = self.parse_where_clause();

        let (fields, is_unit, is_tuple) = match self.peek() {
            TokenKind::LBrace => {
                self.bump();
                let mut fields = Vec::new();
                while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                    let vis = self.parse_visibility();
                    let ident = self.ident_from_token();
                    self.bump();
                    self.expect(&TokenKind::Colon, "`:`");
                    let ty = self.parse_ty();
                    let span = self.current_span();
                    fields.push(StructField {
                        vis,
                        ident: Some(ident),
                        ty,
                        span,
                    });
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RBrace, "`}`");
                (fields, false, false)
            }
            TokenKind::LParen => {
                self.bump();
                let mut fields = Vec::new();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    let vis = self.parse_visibility();
                    let ty = self.parse_ty();
                    let span = self.current_span();
                    fields.push(StructField {
                        vis,
                        ident: None,
                        ty,
                        span,
                    });
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen, "`)`");
                self.expect(&TokenKind::Semicolon, "`;`");
                (fields, false, true)
            }
            TokenKind::Semicolon => {
                self.bump();
                (Vec::new(), true, false)
            }
            _ => {
                self.errors.push(crate::parser::ParseError::new(
                    format!(
                        "expected `{{`, `(`, or `;` after struct, found {}",
                        self.peek()
                    ),
                    self.current_span(),
                ));
                (Vec::new(), true, false)
            }
        };

        StructDecl {
            ident,
            generics: Generics {
                params: generics,
                where_clause,
                span: Span::DUMMY,
            },
            fields,
            is_unit,
            is_tuple,
            span: Span::DUMMY,
        }
    }

    fn parse_enum(&mut self) -> EnumDecl {
        self.bump(); // enum
        let ident = self.ident_from_token();
        self.bump();
        let generics = self.parse_generics();
        let where_clause = self.parse_where_clause();
        self.expect(&TokenKind::LBrace, "`{`");

        let mut variants = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let span = self.current_span();
            let ident = self.ident_from_token();
            self.bump();
            let data = match self.peek() {
                TokenKind::LParen => {
                    self.bump();
                    let mut fields = Vec::new();
                    while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                        let vis = self.parse_visibility();
                        let ty = self.parse_ty();
                        let fspan = self.current_span();
                        fields.push(StructField {
                            vis,
                            ident: None,
                            ty,
                            span: fspan,
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RParen, "`)`");
                    VariantData::Tuple(fields, span)
                }
                TokenKind::LBrace => {
                    self.bump();
                    let mut fields = Vec::new();
                    while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                        let vis = self.parse_visibility();
                        let fident = self.ident_from_token();
                        self.bump();
                        self.expect(&TokenKind::Colon, "`:`");
                        let ty = self.parse_ty();
                        let fspan = self.current_span();
                        fields.push(StructField {
                            vis,
                            ident: Some(fident),
                            ty,
                            span: fspan,
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RBrace, "`}`");
                    VariantData::Struct(fields, span)
                }
                _ => VariantData::Unit(span),
            };
            variants.push(EnumVariant { ident, data, span });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "`}`");

        EnumDecl {
            ident,
            generics: Generics {
                params: generics,
                where_clause,
                span: Span::DUMMY,
            },
            variants,
            span: Span::DUMMY,
        }
    }

    fn parse_trait(&mut self) -> TraitDecl {
        self.bump(); // trait
        let ident = self.expect_ident("trait name");
        let generics = self.parse_generics();
        // Supertraits: `: Bound + Bound`
        let mut supertraits = Vec::new();
        if *self.peek() == TokenKind::Colon {
            self.bump();
            supertraits = self.parse_type_bounds();
        }
        let where_clause = self.parse_where_clause();
        self.expect(&TokenKind::LBrace, "`{`");
        let mut items = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let vis = self.parse_visibility();
            let attrs = self.parse_outer_attrs();
            let item_span = self.current_span();
            let item = match self.peek() {
                TokenKind::KwFn => {
                    let fn_decl = self.parse_fn(false, Abi::Landin);
                    Some(TraitItem::Fn(
                        fn_decl.ident,
                        fn_decl.generics,
                        fn_decl.sig,
                        fn_decl.body,
                    ))
                }
                // unsafe fn inside a trait — `trait Foo { unsafe fn bar(); }`
                TokenKind::KwUnsafe if matches!(self.peek_at(1), TokenKind::KwFn) => {
                    self.bump(); // consume `unsafe`
                    let fn_decl = self.parse_fn(true, Abi::Landin);
                    Some(TraitItem::Fn(
                        fn_decl.ident,
                        fn_decl.generics,
                        fn_decl.sig,
                        fn_decl.body,
                    ))
                }
                TokenKind::KwType => {
                    self.bump();
                    let name = self.expect_ident("associated type name");
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
                    self.expect(&TokenKind::Semicolon, "`;`");
                    Some(TraitItem::Type(name, bounds, default))
                }
                TokenKind::KwConst => {
                    self.bump();
                    let name = self.expect_ident("associated const name");
                    self.expect(&TokenKind::Colon, "`:`");
                    let ty = self.parse_ty();
                    let default = if *self.peek() == TokenKind::Eq {
                        self.bump();
                        Some(self.parse_expr())
                    } else {
                        None
                    };
                    self.expect(&TokenKind::Semicolon, "`;`");
                    Some(TraitItem::Const(name, ty, default))
                }
                _ => {
                    self.errors.push(crate::parser::ParseError::new(
                        format!("expected trait item (fn/type/const), found {}", self.peek()),
                        item_span,
                    ));
                    self.bump();
                    None
                }
            };
            let _ = (vis, attrs); // trait item vis/attrs not yet in AST; attach in Stage 1
            if let Some(i) = item {
                items.push(i);
            }
        }
        self.expect(&TokenKind::RBrace, "`}`");

        TraitDecl {
            ident,
            generics: Generics {
                params: generics,
                where_clause,
                span: self.current_span(),
            },
            supertraits,
            items,
            span: self.current_span(),
        }
    }

    fn parse_impl(&mut self) -> ImplDecl {
        self.bump(); // impl
        let generics = self.parse_generics();
        let self_ty = self.parse_ty();
        let of_trait = if *self.peek() == TokenKind::KwFor {
            self.bump();
            let path = self.parse_path();
            Some(path)
        } else {
            None
        };
        let where_clause = self.parse_where_clause();
        self.expect(&TokenKind::LBrace, "`{`");
        let mut items = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            if let Some(item) = self.parse_item() {
                items.push(item);
            } else {
                self.recover();
            }
        }
        self.expect(&TokenKind::RBrace, "`}`");

        ImplDecl {
            generics: Generics {
                params: generics,
                where_clause,
                span: Span::DUMMY,
            },
            of_trait,
            self_ty,
            items,
            span: Span::DUMMY,
        }
    }

    fn parse_type_alias(&mut self) -> TypeAliasDecl {
        self.bump(); // type
        let ident = self.ident_from_token();
        self.bump();
        let generics = self.parse_generics();
        let where_clause = self.parse_where_clause();
        self.expect(&TokenKind::Eq, "`=`");
        let ty = self.parse_ty();
        self.expect(&TokenKind::Semicolon, "`;`");
        TypeAliasDecl {
            ident,
            generics: Generics {
                params: generics,
                where_clause,
                span: Span::DUMMY,
            },
            ty,
            span: Span::DUMMY,
        }
    }

    /// Parse `extern "C" { ... }` block OR `extern "C" fn foo() {}` standalone item.
    ///
    /// Per 02-grammar.md §3.1, the block form is canonical. Rust also supports
    /// the standalone form `extern "C" fn foo() {}` — we accept both for
    /// interoperability, dispatching based on whether `fn` follows the abi.
    /// Returns ExternBlock for the block form; for the standalone fn form,
    /// we wrap the fn in an ExternBlock with a single item (a compromise
    /// until the AST has a separate ExternFn item kind).
    fn parse_extern_block_or_fn(&mut self) -> ExternBlock {
        self.bump(); // extern
        let abi = {
            if let TokenKind::StrLit(sym) = self.peek().clone() {
                self.bump();
                let s = self.interner.resolve(&sym).to_string();
                match s.as_str() {
                    "C" => Abi::C,
                    "System" => Abi::System,
                    _ => Abi::Landin,
                }
            } else {
                Abi::Landin
            }
        };
        // Standalone extern fn: `extern "C" fn foo() {}`
        if *self.peek() == TokenKind::KwFn {
            let fn_decl = self.parse_fn(false, abi);
            let item = Item {
                vis: Visibility::Private,
                attrs: Vec::new(),
                kind: ItemKind::Fn(fn_decl),
                span: self.current_span(),
            };
            return ExternBlock {
                abi,
                items: vec![item],
                span: self.current_span(),
            };
        }
        // Block form: `extern "C" { ... }`
        self.expect(&TokenKind::LBrace, "`{`");
        let mut items = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            if let Some(item) = self.parse_item() {
                items.push(item);
            } else {
                self.recover();
            }
        }
        self.expect(&TokenKind::RBrace, "`}`");
        ExternBlock {
            abi,
            items,
            span: self.current_span(),
        }
    }

    fn parse_mod(&mut self) -> ModDecl {
        self.bump(); // mod
        let ident = self.expect_ident("module name");
        let span = self.current_span();
        if *self.peek() == TokenKind::LBrace {
            self.bump();
            let mut items = Vec::new();
            while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                if let Some(item) = self.parse_item() {
                    items.push(item);
                } else {
                    self.recover();
                }
            }
            self.expect(&TokenKind::RBrace, "`}`");
            ModDecl::Inline { ident, items, span }
        } else {
            self.expect(&TokenKind::Semicolon, "`;`");
            ModDecl::Loaded { ident, span }
        }
    }

    fn parse_use(&mut self) -> UseDecl {
        self.bump(); // use
        let span = self.current_span();
        let tree = self.parse_use_tree();
        self.expect(&TokenKind::Semicolon, "`;`");
        UseDecl { tree, span }
    }

    /// Parse a use tree.
    ///
    /// Grammar:
    ///   use_tree := path ( "::" ("{" use_tree ("," use_tree)* "}" | "*") )?
    ///             | "{" use_tree ("," use_tree)* "}"
    ///             | path "as" ident
    fn parse_use_tree(&mut self) -> UseTree {
        let path = self.parse_path();
        // Glob: `path::*`
        if *self.peek() == TokenKind::PathSep && *self.peek_at(1) == TokenKind::Star {
            self.bump(); // ::
            self.bump(); // *
            return UseTree::Glob(path);
        }
        // Group: `path::{a, b, c}`
        if *self.peek() == TokenKind::PathSep && *self.peek_at(1) == TokenKind::LBrace {
            self.bump(); // ::
            self.bump(); // {
            let mut children = Vec::new();
            while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                children.push(self.parse_use_tree());
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RBrace, "`}`");
            return UseTree::Path {
                prefix: path,
                children,
            };
        }
        // Alias: `path as name`
        let alias = if *self.peek() == TokenKind::KwAs {
            self.bump();
            Some(self.expect_ident("use alias name"))
        } else {
            None
        };
        UseTree::Leaf(path, alias)
    }

    // --- Types ---

    fn parse_ty(&mut self) -> Ty {
        let span = self.current_span();
        match self.peek().clone() {
            // Basic primitive types
            TokenKind::Ident(sym) => {
                let name = self.interner.resolve(&sym).to_string();
                match name.as_str() {
                    "bool" => {
                        self.bump();
                        return Ty::Bool(span);
                    }
                    "char" => {
                        self.bump();
                        return Ty::Char(span);
                    }
                    "i8" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::I8, span);
                    }
                    "i16" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::I16, span);
                    }
                    "i32" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::I32, span);
                    }
                    "i64" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::I64, span);
                    }
                    "i128" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::I128, span);
                    }
                    "isize" => {
                        self.bump();
                        return Ty::Int(ast::IntTy::Isize, span);
                    }
                    "u8" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::U8, span);
                    }
                    "u16" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::U16, span);
                    }
                    "u32" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::U32, span);
                    }
                    "u64" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::U64, span);
                    }
                    "u128" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::U128, span);
                    }
                    "usize" => {
                        self.bump();
                        return Ty::Uint(ast::UintTy::Usize, span);
                    }
                    "f32" => {
                        self.bump();
                        return Ty::Float(ast::FloatTy::F32, span);
                    }
                    "f64" => {
                        self.bump();
                        return Ty::Float(ast::FloatTy::F64, span);
                    }
                    _ => {} // Fall through to path type
                }
                let path = self.parse_path();
                Ty::Path(QSelf::default(), path, span)
            }
            TokenKind::Not => {
                self.bump();
                Ty::Never(span)
            }
            TokenKind::LParen => {
                self.bump();
                let mut tys = Vec::new();
                if *self.peek() != TokenKind::RParen {
                    tys.push(self.parse_ty());
                    if *self.peek() == TokenKind::Comma {
                        self.bump();
                        if *self.peek() != TokenKind::RParen {
                            tys.push(self.parse_ty());
                            while self.eat(&TokenKind::Comma) {
                                if *self.peek() == TokenKind::RParen {
                                    break;
                                }
                                tys.push(self.parse_ty());
                            }
                        }
                    }
                }
                self.expect(&TokenKind::RParen, "`)`");
                Ty::Tuple(tys, span)
            }
            TokenKind::LBracket => {
                self.bump();
                let ty = self.parse_ty();
                if *self.peek() == TokenKind::Semicolon {
                    self.bump();
                    let count = self.parse_expr();
                    self.expect(&TokenKind::RBracket, "`]`");
                    Ty::Array(Box::new(ty), Box::new(count), span)
                } else {
                    self.expect(&TokenKind::RBracket, "`]`");
                    Ty::Slice(Box::new(ty), span)
                }
            }
            TokenKind::And => {
                self.bump();
                // Preserve lifetime in `&'a T` — Stage 1 lifetime elision needs this.
                let lifetime: Option<Lifetime> =
                    if let TokenKind::Lifetime(sym) = self.peek().clone() {
                        let l_span = self.current_span();
                        self.bump();
                        Some(Lifetime {
                            ident: Ident::new(sym, l_span),
                            span: l_span,
                        })
                    } else {
                        None
                    };
                let mutability = if *self.peek() == TokenKind::KwMut {
                    self.bump();
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                let ty = self.parse_ty();
                Ty::Ref(lifetime, mutability, Box::new(ty), span)
            }
            TokenKind::Star => {
                self.bump();
                // *const or *mut
                let mutability = if *self.peek() == TokenKind::KwMut {
                    self.bump();
                    Mutability::Mutable
                } else {
                    self.bump(); // const
                    Mutability::Immutable
                };
                let ty = self.parse_ty();
                Ty::Ptr(mutability, Box::new(ty), span)
            }
            TokenKind::KwFn => {
                self.bump();
                let is_unsafe = false; // `unsafe fn` type handled by KwUnsafe arm below
                let _ = is_unsafe;
                self.expect(&TokenKind::LParen, "`(`");
                let mut inputs = Vec::new();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    // Fn pointer params are types only (no patterns)
                    inputs.push(self.parse_ty());
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen, "`)`");
                let output = if *self.peek() == TokenKind::Arrow {
                    self.bump();
                    Box::new(self.parse_ty())
                } else {
                    Box::new(Ty::Tuple(Vec::new(), span))
                };
                Ty::FnPtr {
                    inputs,
                    output,
                    abi: Abi::Landin,
                    is_unsafe: false,
                    span,
                }
            }
            // `unsafe fn` as a type (fn pointer that is unsafe)
            TokenKind::KwUnsafe if matches!(self.peek_at(1), TokenKind::KwFn) => {
                self.bump(); // unsafe
                self.bump(); // fn
                self.expect(&TokenKind::LParen, "`(`");
                let mut inputs = Vec::new();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    inputs.push(self.parse_ty());
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen, "`)`");
                let output = if *self.peek() == TokenKind::Arrow {
                    self.bump();
                    Box::new(self.parse_ty())
                } else {
                    Box::new(Ty::Tuple(Vec::new(), span))
                };
                Ty::FnPtr {
                    inputs,
                    output,
                    abi: Abi::Landin,
                    is_unsafe: true,
                    span,
                }
            }
            // impl Trait — `impl Clone + Default`
            TokenKind::KwImpl => {
                self.bump();
                let bounds = self.parse_type_bounds();
                Ty::ImplTrait(bounds, span)
            }
            // dyn Trait — `dyn Display + Send`
            TokenKind::KwDyn => {
                self.bump();
                let bounds = self.parse_type_bounds();
                // Optional lifetime before `dyn`: `'a dyn Trait` (rare; usually `&(dyn Trait + 'a)`)
                Ty::TraitObject {
                    bounds,
                    lifetime: None,
                    span,
                }
            }
            TokenKind::Underscore => {
                self.bump();
                Ty::Infer(span)
            }
            _ => {
                // Path type (including KwSelf_ / KwSelfType as the type itself)
                let path = self.parse_path();
                Ty::Path(QSelf::default(), path, span)
            }
        }
    }

    // --- Paths ---

    fn parse_path(&mut self) -> Path {
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
        let args = self.try_parse_generic_args();
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
            let args = self.try_parse_generic_args();
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
    fn try_parse_generic_args(&mut self) -> Option<GenericArgs> {
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
    fn parse_pat(&mut self) -> Pat {
        self.parse_or_pat()
    }

    /// Parse an or-pattern: `pat | pat | pat`.
    fn parse_or_pat(&mut self) -> Pat {
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
    fn parse_pat_no_or(&mut self) -> Pat {
        let span = self.current_span();
        match self.peek().clone() {
            TokenKind::Underscore => {
                self.bump();
                Pat::Wild(span)
            }
            TokenKind::KwMut => {
                self.bump();
                let ident = self.expect_ident("pattern binding name");
                Pat::Ident(BindingMode::ByValue, ident, None)
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
                let path = self.parse_path();
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
                                (Pat::Ident(BindingMode::ByValue, field_ident, None), true)
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
                    _ => Pat::Path(path, span),
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
                                BindingMode::ByValue,
                                ident,
                                Some(Box::new(sub_pat)),
                            );
                        }
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
                Pat::Ident(BindingMode::ByValue, ident, None)
            }
        }
    }

    // --- Blocks and statements ---

    fn parse_block(&mut self) -> Block {
        let span = self.current_span();
        self.expect(&TokenKind::LBrace, "`{`");
        let mut stmts = Vec::new();
        let mut trailing_expr = None;

        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            // Progress guard: snapshot the position before attempting to parse
            // a statement. If `parse_expr` returns without consuming any token
            // (which happens for unsupported syntax like `Point { x: 1 }` when
            // struct literals aren't yet supported, or `if let` patterns that
            // fall through to `parse_path` returning an empty path), we MUST
            // force-advance to avoid an infinite loop / OOM kill.
            let pos_before = self.pos;

            // Check for let statement
            if *self.peek() == TokenKind::KwLet {
                let stmt = self.parse_let();
                stmts.push(Stmt::Local(stmt));
                continue;
            }

            // Parse expression
            let expr = self.parse_expr();

            // Progress guard: if no token was consumed, force-advance and emit
            // an error so the user gets a useful message instead of a hang.
            if self.pos == pos_before {
                let bad_span = self.current_span();
                self.errors.push(crate::parser::ParseError::new(
                    format!(
                        "could not parse expression starting at `{}` (parser made no progress — \
                         this syntax may be unsupported in Stage 0)",
                        self.peek(),
                    ),
                    bad_span,
                ));
                self.bump();
                // Skip the bad token and continue; do not push a stmt.
                continue;
            }

            let has_semi = self.eat(&TokenKind::Semicolon);

            if !has_semi && *self.peek() == TokenKind::RBrace {
                // Trailing expression
                trailing_expr = Some(Box::new(expr));
                break;
            }

            stmts.push(Stmt::Expr(expr, has_semi));
        }

        self.expect(&TokenKind::RBrace, "`}`");
        Block {
            stmts,
            expr: trailing_expr,
            span,
        }
    }

    fn parse_let(&mut self) -> LocalDecl {
        let span = self.current_span();
        self.bump(); // let
        let pat = self.parse_pat();
        let ty = if *self.peek() == TokenKind::Colon {
            self.bump();
            Some(self.parse_ty())
        } else {
            None
        };
        let init = if *self.peek() == TokenKind::Eq {
            self.bump();
            Some(self.parse_expr())
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon, "`;`");
        LocalDecl {
            pat,
            ty,
            init,
            span,
        }
    }

    // --- Expressions (Pratt parser) ---

    /// Pratt operator precedence (higher = binds tighter).
    /// Based on 02-grammar.md §2 Pratt table.
    fn binop_bp(op: &TokenKind) -> Option<(BinOp, u8)> {
        Some(match op {
            TokenKind::OrOr => (BinOp::Or, 1),
            TokenKind::AndAnd => (BinOp::And, 2),
            TokenKind::EqEq => (BinOp::Eq, 3),
            TokenKind::NotEq => (BinOp::Ne, 3),
            TokenKind::Lt => (BinOp::Lt, 3),
            TokenKind::Gt => (BinOp::Gt, 3),
            TokenKind::LtEq => (BinOp::Le, 3),
            TokenKind::GtEq => (BinOp::Ge, 3),
            TokenKind::Or => (BinOp::BitOr, 4),
            TokenKind::Caret => (BinOp::BitXor, 5),
            TokenKind::And => (BinOp::BitAnd, 6),
            TokenKind::Shl => (BinOp::Shl, 7),
            TokenKind::Shr => (BinOp::Shr, 7),
            TokenKind::Plus => (BinOp::Add, 8),
            TokenKind::Minus => (BinOp::Sub, 8),
            TokenKind::Star => (BinOp::Mul, 9),
            TokenKind::Slash => (BinOp::Div, 9),
            TokenKind::Percent => (BinOp::Rem, 9),
            _ => return None,
        })
    }

    fn assign_op(op: &TokenKind) -> Option<Option<BinOp>> {
        Some(match op {
            TokenKind::Eq => None,
            TokenKind::PlusEq => Some(BinOp::Add),
            TokenKind::MinusEq => Some(BinOp::Sub),
            TokenKind::StarEq => Some(BinOp::Mul),
            TokenKind::SlashEq => Some(BinOp::Div),
            TokenKind::PercentEq => Some(BinOp::Rem),
            TokenKind::AndEq => Some(BinOp::BitAnd),
            TokenKind::OrEq => Some(BinOp::BitOr),
            TokenKind::CaretEq => Some(BinOp::BitXor),
            TokenKind::ShlEq => Some(BinOp::Shl),
            TokenKind::ShrEq => Some(BinOp::Shr),
            _ => return None,
        })
    }

    pub fn parse_expr(&mut self) -> Expr {
        self.parse_assign_expr()
    }

    fn parse_assign_expr(&mut self) -> Expr {
        let lhs = self.parse_range_expr();
        let span = lhs.span();

        if let Some(op) = Self::assign_op(self.peek()) {
            self.bump();
            let rhs = self.parse_assign_expr();
            return Expr::Assign {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                op,
                span,
            };
        }
        lhs
    }

    fn parse_range_expr(&mut self) -> Expr {
        // Check for range start..
        let lhs = self.parse_or_expr();
        let span = lhs.span();

        match self.peek() {
            TokenKind::DotDot => {
                self.bump();
                // Check if there's a rhs
                let end = if self.is_expr_start() {
                    Some(Box::new(self.parse_or_expr()))
                } else {
                    None
                };
                Expr::Range {
                    start: Some(Box::new(lhs)),
                    end,
                    end_kind: RangeEnd::Excluded,
                    span,
                }
            }
            TokenKind::DotDotEq => {
                self.bump();
                let end = Some(Box::new(self.parse_or_expr()));
                Expr::Range {
                    start: Some(Box::new(lhs)),
                    end,
                    end_kind: RangeEnd::Included,
                    span,
                }
            }
            _ => lhs,
        }
    }

    fn parse_or_expr(&mut self) -> Expr {
        let mut lhs = self.parse_and_expr();
        while let Some((op, _bp)) = Self::binop_bp(self.peek()) {
            if op != BinOp::Or {
                break;
            }
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_and_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    fn parse_and_expr(&mut self) -> Expr {
        let mut lhs = self.parse_cmp_expr();
        while let Some((op, _bp)) = Self::binop_bp(self.peek()) {
            if op != BinOp::And {
                break;
            }
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_cmp_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    fn parse_cmp_expr(&mut self) -> Expr {
        let mut lhs = self.parse_bitor_expr();
        while let Some((op, _bp)) = Self::binop_bp(self.peek()) {
            if !matches!(
                op,
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
            ) {
                break;
            }
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_bitor_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    fn parse_bitor_expr(&mut self) -> Expr {
        let mut lhs = self.parse_bitxor_expr();
        while *self.peek() == TokenKind::Or {
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_bitxor_expr();
            lhs = Expr::Binary {
                op: BinOp::BitOr,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    fn parse_bitxor_expr(&mut self) -> Expr {
        let mut lhs = self.parse_bitand_expr();
        while *self.peek() == TokenKind::Caret {
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_bitand_expr();
            lhs = Expr::Binary {
                op: BinOp::BitXor,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    fn parse_bitand_expr(&mut self) -> Expr {
        let mut lhs = self.parse_shift_expr();
        while *self.peek() == TokenKind::And {
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_shift_expr();
            lhs = Expr::Binary {
                op: BinOp::BitAnd,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    fn parse_shift_expr(&mut self) -> Expr {
        let mut lhs = self.parse_add_expr();
        while matches!(self.peek(), TokenKind::Shl | TokenKind::Shr) {
            let (op, _) = Self::binop_bp(self.peek()).unwrap();
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_add_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    fn parse_add_expr(&mut self) -> Expr {
        let mut lhs = self.parse_mul_expr();
        while matches!(self.peek(), TokenKind::Plus | TokenKind::Minus) {
            let (op, _) = Self::binop_bp(self.peek()).unwrap();
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_mul_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    fn parse_mul_expr(&mut self) -> Expr {
        let mut lhs = self.parse_cast_expr();
        while matches!(
            self.peek(),
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent
        ) {
            let (op, _) = Self::binop_bp(self.peek()).unwrap();
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_cast_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    fn parse_cast_expr(&mut self) -> Expr {
        let lhs = self.parse_unary_expr();
        let span = lhs.span();
        if *self.peek() == TokenKind::KwAs {
            self.bump();
            let ty = self.parse_ty();
            return Expr::Cast {
                expr: Box::new(lhs),
                ty,
                span,
            };
        }
        lhs
    }

    fn parse_unary_expr(&mut self) -> Expr {
        let span = self.current_span();
        match self.peek() {
            TokenKind::Minus => {
                self.bump();
                let expr = self.parse_unary_expr();
                Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                    span,
                }
            }
            TokenKind::Not => {
                self.bump();
                let expr = self.parse_unary_expr();
                Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                    span,
                }
            }
            TokenKind::Star => {
                self.bump();
                let expr = self.parse_unary_expr();
                Expr::Unary {
                    op: UnaryOp::Deref,
                    expr: Box::new(expr),
                    span,
                }
            }
            TokenKind::And => {
                self.bump();
                let mutability = if *self.peek() == TokenKind::KwMut {
                    self.bump();
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                let expr = self.parse_unary_expr();
                Expr::AddrOf {
                    mutability,
                    expr: Box::new(expr),
                    span,
                }
            }
            _ => self.parse_postfix_expr(),
        }
    }

    fn parse_postfix_expr(&mut self) -> Expr {
        let mut expr = self.parse_primary_expr();

        loop {
            let span = expr.span();
            match self.peek() {
                TokenKind::Dot => {
                    self.bump();
                    // Field access or method call or tuple field
                    match self.peek() {
                        TokenKind::Ident(_) => {
                            let ident = self.ident_from_token();
                            self.bump();
                            // Check for method call
                            if *self.peek() == TokenKind::LParen {
                                self.bump();
                                let mut args = Vec::new();
                                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                                    args.push(self.parse_expr());
                                    if !self.eat(&TokenKind::Comma) {
                                        break;
                                    }
                                }
                                self.expect(&TokenKind::RParen, "`)`");
                                expr = Expr::MethodCall {
                                    receiver: Box::new(expr),
                                    method: ident,
                                    args,
                                    generic_args: None,
                                    span,
                                };
                            } else {
                                expr = Expr::Field {
                                    receiver: Box::new(expr),
                                    ident,
                                    span,
                                };
                            }
                        }
                        TokenKind::IntLit(_, _) => {
                            // Tuple field access: t.0
                            self.bump();
                            expr = Expr::Field {
                                receiver: Box::new(expr),
                                ident: Ident::new(Spur::default(), span),
                                span,
                            };
                        }
                        _ => {
                            self.errors.push(crate::parser::ParseError::new(
                                format!("expected field name after `.`, found {}", self.peek()),
                                span,
                            ));
                            break;
                        }
                    }
                }
                TokenKind::LParen => {
                    self.bump();
                    let mut args = Vec::new();
                    while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                        args.push(self.parse_expr());
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RParen, "`)`");
                    expr = Expr::Call {
                        func: Box::new(expr),
                        args,
                        span,
                    };
                }
                TokenKind::LBracket => {
                    self.bump();
                    let index = self.parse_expr();
                    self.expect(&TokenKind::RBracket, "`]`");
                    expr = Expr::Index {
                        receiver: Box::new(expr),
                        index: Box::new(index),
                        span,
                    };
                }
                TokenKind::Question => {
                    self.bump();
                    expr = Expr::Try {
                        expr: Box::new(expr),
                        span,
                    };
                }
                _ => break,
            }
        }
        expr
    }

    fn parse_primary_expr(&mut self) -> Expr {
        let span = self.current_span();
        match self.peek().clone() {
            TokenKind::IntLit(val, suffix) => {
                self.bump();
                let lit_kind = match suffix {
                    Some(crate::lexer::token::IntTy::I8) => LitKind::Int(val, Some(ast::IntTy::I8)),
                    Some(crate::lexer::token::IntTy::I16) => {
                        LitKind::Int(val, Some(ast::IntTy::I16))
                    }
                    Some(crate::lexer::token::IntTy::I32) => {
                        LitKind::Int(val, Some(ast::IntTy::I32))
                    }
                    Some(crate::lexer::token::IntTy::I64) => {
                        LitKind::Int(val, Some(ast::IntTy::I64))
                    }
                    Some(crate::lexer::token::IntTy::I128) => {
                        LitKind::Int(val, Some(ast::IntTy::I128))
                    }
                    Some(crate::lexer::token::IntTy::Isize) => {
                        LitKind::Int(val, Some(ast::IntTy::Isize))
                    }
                    Some(crate::lexer::token::IntTy::U8) => {
                        LitKind::Uint(val, Some(ast::UintTy::U8))
                    }
                    Some(crate::lexer::token::IntTy::U16) => {
                        LitKind::Uint(val, Some(ast::UintTy::U16))
                    }
                    Some(crate::lexer::token::IntTy::U32) => {
                        LitKind::Uint(val, Some(ast::UintTy::U32))
                    }
                    Some(crate::lexer::token::IntTy::U64) => {
                        LitKind::Uint(val, Some(ast::UintTy::U64))
                    }
                    Some(crate::lexer::token::IntTy::U128) => {
                        LitKind::Uint(val, Some(ast::UintTy::U128))
                    }
                    Some(crate::lexer::token::IntTy::Usize) => {
                        LitKind::Uint(val, Some(ast::UintTy::Usize))
                    }
                    None => LitKind::Int(val, None),
                };
                Expr::Lit(lit_kind, span)
            }
            TokenKind::FloatLit(val, suffix) => {
                self.bump();
                Expr::Lit(
                    LitKind::Float(val, suffix.map(|_| crate::ast::FloatTy::F64)),
                    span,
                )
            }
            TokenKind::KwTrue => {
                self.bump();
                Expr::Lit(LitKind::Bool(true), span)
            }
            TokenKind::KwFalse => {
                self.bump();
                Expr::Lit(LitKind::Bool(false), span)
            }
            TokenKind::CharLit(c) => {
                self.bump();
                Expr::Lit(LitKind::Char(c), span)
            }
            TokenKind::StrLit(sym) => {
                self.bump();
                Expr::Lit(LitKind::Str(sym), span)
            }
            TokenKind::ByteLit(b) => {
                self.bump();
                Expr::Lit(LitKind::Byte(b), span)
            }
            TokenKind::ByteStrLit(sym) => {
                self.bump();
                Expr::Lit(LitKind::ByteStr(sym), span)
            }
            TokenKind::LParen => {
                self.bump();
                if *self.peek() == TokenKind::RParen {
                    self.bump();
                    return Expr::Unit(span);
                }
                let first = self.parse_expr();
                if *self.peek() == TokenKind::Comma {
                    self.bump();
                    let mut elems = vec![first];
                    while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                        elems.push(self.parse_expr());
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RParen, "`)`");
                    return Expr::Tuple { elems, span };
                }
                self.expect(&TokenKind::RParen, "`)`");
                first
            }
            TokenKind::LBracket => {
                self.bump();
                if *self.peek() == TokenKind::RBracket {
                    self.bump();
                    return Expr::Array {
                        elems: Vec::new(),
                        span,
                    };
                }
                let first = self.parse_expr();
                if *self.peek() == TokenKind::Semicolon {
                    self.bump();
                    let count = self.parse_expr();
                    self.expect(&TokenKind::RBracket, "`]`");
                    return Expr::Repeat {
                        elem: Box::new(first),
                        count: Box::new(count),
                        span,
                    };
                }
                let mut elems = vec![first];
                while self.eat(&TokenKind::Comma) {
                    if *self.peek() == TokenKind::RBracket {
                        break;
                    }
                    elems.push(self.parse_expr());
                }
                self.expect(&TokenKind::RBracket, "`]`");
                Expr::Array { elems, span }
            }
            // Note: Block expressions { ... } are NOT parsed here to avoid
            // ambiguity with for/match/while/struct literal blocks.
            // Use explicit blocks via parse_block() in control flow expressions.
            TokenKind::KwIf => self.parse_if_expr(),
            TokenKind::KwMatch => self.parse_match_expr(),
            TokenKind::KwLoop => {
                self.bump();
                let body = self.parse_block();
                Expr::Loop { body, span }
            }
            TokenKind::KwWhile => {
                self.bump();
                // while let Pat = expr block
                let cond = if *self.peek() == TokenKind::KwLet {
                    self.bump(); // let
                    let _pat = self.parse_pat();
                    self.expect(&TokenKind::Eq, "`=`");
                    let prev = self.no_struct_literal;
                    self.no_struct_literal = true;
                    let scrutinee = self.parse_expr();
                    self.no_struct_literal = prev;
                    self.errors.push(crate::parser::ParseError::new(
                        "`while let` patterns are not yet supported in Stage 0 (will be added in Stage 1)".to_string(),
                        span,
                    ));
                    scrutinee
                } else {
                    let prev = self.no_struct_literal;
                    self.no_struct_literal = true;
                    let c = self.parse_expr();
                    self.no_struct_literal = prev;
                    c
                };
                let body = self.parse_block();
                Expr::While {
                    cond: Box::new(cond),
                    body,
                    span,
                }
            }
            TokenKind::KwFor => {
                self.bump();
                let pat = self.parse_pat();
                self.expect(&TokenKind::KwIn, "`in`");
                let prev = self.no_struct_literal;
                self.no_struct_literal = true;
                let iter = self.parse_expr();
                self.no_struct_literal = prev;
                let body = self.parse_block();
                Expr::For {
                    pat,
                    iter: Box::new(iter),
                    body,
                    span,
                }
            }
            TokenKind::KwReturn => {
                self.bump();
                let expr = if self.is_expr_start() {
                    Some(Box::new(self.parse_expr()))
                } else {
                    None
                };
                Expr::Return { expr, span }
            }
            TokenKind::KwBreak => {
                self.bump();
                let expr = if self.is_expr_start() {
                    Some(Box::new(self.parse_expr()))
                } else {
                    None
                };
                Expr::Break { expr, span }
            }
            TokenKind::KwContinue => {
                self.bump();
                Expr::Continue { span }
            }
            TokenKind::KwUnsafe => {
                self.bump();
                let block = self.parse_block();
                Expr::Unsafe(block, span)
            }
            // Block expression: `{ stmts; trailing_expr }`.
            // We do allow this in expression position now (Round 2e fix);
            // the `no_struct_literal` flag handles the if/while/for/match
            // condition disambiguation separately, so a `{` following a path
            // is still treated as struct literal, but a bare `{` starting an
            // expression is a block.
            TokenKind::LBrace => {
                let block = self.parse_block();
                Expr::Block(block, span)
            }
            TokenKind::Or | TokenKind::OrOr => {
                // Closure: |args| expr or || expr (empty params)
                // Possibly preceded by `move` keyword — handled below in KwMove arm.
                let is_double = *self.peek() == TokenKind::OrOr;
                self.bump(); // | or ||
                let mut params = Vec::new();
                if !is_double {
                    // Single |: parse params until closing |
                    while !matches!(self.peek(), TokenKind::Or | TokenKind::Eof) {
                        let pat = self.parse_pat();
                        let ty = if *self.peek() == TokenKind::Colon {
                            self.bump();
                            Some(self.parse_ty())
                        } else {
                            None
                        };
                        params.push(Param {
                            pat,
                            ty: ty.unwrap_or(Ty::Infer(span)),
                            attrs: Vec::new(),
                            is_self: false,
                            span,
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.eat(&TokenKind::Or); // closing |
                }
                let body = self.parse_expr();
                Expr::Closure {
                    is_move: false,
                    params,
                    body: Box::new(body),
                    span,
                }
            }
            // move closure: `move |args| expr` or `move || expr`
            TokenKind::KwMove if matches!(self.peek_at(1), TokenKind::Or | TokenKind::OrOr) => {
                self.bump(); // move
                let is_double = *self.peek() == TokenKind::OrOr;
                self.bump(); // | or ||
                let mut params = Vec::new();
                if !is_double {
                    while !matches!(self.peek(), TokenKind::Or | TokenKind::Eof) {
                        let pat = self.parse_pat();
                        let ty = if *self.peek() == TokenKind::Colon {
                            self.bump();
                            Some(self.parse_ty())
                        } else {
                            None
                        };
                        params.push(Param {
                            pat,
                            ty: ty.unwrap_or(Ty::Infer(span)),
                            attrs: Vec::new(),
                            is_self: false,
                            span,
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.eat(&TokenKind::Or); // closing |
                }
                let body = self.parse_expr();
                Expr::Closure {
                    is_move: true,
                    params,
                    body: Box::new(body),
                    span,
                }
            }
            _ => {
                // Path expression — possibly struct literal `Foo { x: 1, y: 2 }`
                // or macro call `ident!(...)`.
                let path = self.parse_path();
                let path_span = span;
                // Macro call: `!` followed by `(`/`{`/`[`
                if *self.peek() == TokenKind::Not
                    && matches!(
                        self.peek_at(1),
                        TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket
                    )
                {
                    self.bump(); // !
                    let delim = match self.peek() {
                        TokenKind::LParen => MacroDelim::Paren,
                        TokenKind::LBrace => MacroDelim::Brace,
                        TokenKind::LBracket => MacroDelim::Bracket,
                        _ => unreachable!(),
                    };
                    // Skip the macro body tokens for Stage 0 — we just balance
                    // the delimiters. Stage 4 macro expansion will re-parse them.
                    self.skip_delim_group();
                    return Expr::MacroCall {
                        path,
                        delim,
                        span: path_span,
                    };
                }
                // Struct literal: `Path { field: expr, .. }`
                // Disambiguation: struct literals are NOT allowed in `if`/`while`/`for`/`match`
                // condition positions (those `{` belong to the block). We use a
                // `no_struct_literal` flag controlled by the condition-parsing sites.
                if *self.peek() == TokenKind::LBrace && !self.no_struct_literal {
                    self.bump(); // {
                    let mut fields = Vec::new();
                    while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                        // `..base` (struct update syntax)
                        if *self.peek() == TokenKind::DotDot {
                            self.bump();
                            let _base = self.parse_expr();
                            // For Stage 0 we don't store the base expression in the AST;
                            // the existing Expr::Struct doesn't have a base field.
                            // Stage 1 will extend the AST.
                            break;
                        }
                        let field_ident = self.expect_ident("struct literal field name");
                        let (field_expr, is_shorthand_unused) = if *self.peek() == TokenKind::Colon
                        {
                            self.bump();
                            (self.parse_expr(), false)
                        } else {
                            // Shorthand: `field` means `field: field`
                            let shorthand = Expr::Path(
                                None,
                                Path {
                                    segments: vec![PathSegment {
                                        ident: field_ident,
                                        args: None,
                                    }],
                                    leading: PathLeading::None,
                                    span: self.current_span(),
                                },
                                self.current_span(),
                            );
                            (shorthand, true)
                        };
                        let _ = is_shorthand_unused;
                        let f_span = self.current_span();
                        fields.push(ExprField {
                            ident: field_ident,
                            expr: Some(field_expr),
                            span: f_span,
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RBrace, "`}`");
                    return Expr::Struct {
                        path,
                        fields,
                        span: path_span,
                    };
                }
                Expr::Path(None, path, path_span)
            }
        }
    }

    /// Skip a balanced delimiter group: `(...)` / `{...}` / `[...]`.
    /// Used to skip over macro bodies without parsing them as expressions.
    fn skip_delim_group(&mut self) {
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

    fn parse_if_expr(&mut self) -> Expr {
        let span = self.current_span();
        self.bump(); // if
                     // if let Pat = expr block — peek for KwLet after if.
                     // We model `if let P = e { ... }` as `if (let P = e) { ... }` by
                     // wrapping the let-pattern in a special Expr variant. For Stage 0
                     // (no Expr::Let variant in AST), we fall back to parsing the
                     // pat = expr as a regular expression and emit a soft error.
        let cond = if *self.peek() == TokenKind::KwLet {
            // if let pattern
            self.bump(); // let
            let _pat = self.parse_pat();
            self.expect(&TokenKind::Eq, "`=`");
            // Set no_struct_literal so the scrutinee `{` doesn't get eaten.
            let prev = self.no_struct_literal;
            self.no_struct_literal = true;
            let scrutinee = self.parse_expr();
            self.no_struct_literal = prev;
            // For Stage 0 we don't have Expr::Let — emit a soft error and
            // use the scrutinee as the condition so the block parses.
            self.errors.push(crate::parser::ParseError::new(
                "`if let` patterns are not yet supported in Stage 0 (will be added in Stage 1)"
                    .to_string(),
                span,
            ));
            scrutinee
        } else {
            // Regular if: parse cond with no_struct_literal = true
            let prev = self.no_struct_literal;
            self.no_struct_literal = true;
            let c = self.parse_expr();
            self.no_struct_literal = prev;
            c
        };
        let then = self.parse_block();
        let else_ = if *self.peek() == TokenKind::KwElse {
            self.bump();
            // else can be either { block } or if expr
            if *self.peek() == TokenKind::LBrace {
                let block = self.parse_block();
                Some(Box::new(Expr::Block(block, self.current_span())))
            } else {
                Some(Box::new(self.parse_expr()))
            }
        } else {
            None
        };
        Expr::If {
            cond: Box::new(cond),
            then,
            else_,
            span,
        }
    }

    fn parse_match_expr(&mut self) -> Expr {
        let span = self.current_span();
        self.bump(); // match
                     // Scrutinee: no_struct_literal = true (so `{` belongs to match arms)
        let prev = self.no_struct_literal;
        self.no_struct_literal = true;
        let expr = self.parse_expr();
        self.no_struct_literal = prev;
        self.expect(&TokenKind::LBrace, "`{`");
        let mut arms = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let pat = self.parse_pat();
            // Match guard: `if expr`
            let guard = if *self.peek() == TokenKind::KwIf {
                self.bump();
                let prev = self.no_struct_literal;
                self.no_struct_literal = true;
                let g = self.parse_expr();
                self.no_struct_literal = prev;
                Some(g)
            } else {
                None
            };
            self.expect(&TokenKind::FatArrow, "`=>`");
            let body = self.parse_expr();
            self.eat(&TokenKind::Comma);
            let arm_span = self.current_span();
            arms.push(Arm {
                pat,
                guard,
                body: Box::new(body),
                span: arm_span,
            });
        }
        self.expect(&TokenKind::RBrace, "`}`");
        Expr::Match {
            expr: Box::new(expr),
            arms,
            span,
        }
    }

    fn is_expr_start(&self) -> bool {
        matches!(
            self.peek(),
            TokenKind::IntLit(_, _)
                | TokenKind::FloatLit(_, _)
                | TokenKind::KwTrue
                | TokenKind::KwFalse
                | TokenKind::CharLit(_)
                | TokenKind::StrLit(_)
                | TokenKind::ByteLit(_)
                | TokenKind::ByteStrLit(_)
                | TokenKind::RawStrLit(_, _)
                | TokenKind::Ident(_)
                | TokenKind::RawIdent(_)
                | TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::LBrace
                | TokenKind::KwIf
                | TokenKind::KwMatch
                | TokenKind::KwLoop
                | TokenKind::KwWhile
                | TokenKind::KwFor
                | TokenKind::KwReturn
                | TokenKind::KwBreak
                | TokenKind::KwContinue
                | TokenKind::KwUnsafe
                | TokenKind::Minus
                | TokenKind::Not
                | TokenKind::Star
                | TokenKind::And
                | TokenKind::Or
                | TokenKind::OrOr
                | TokenKind::PathSep
                | TokenKind::KwSelf_
                | TokenKind::KwSelfType
                | TokenKind::KwCrate
                | TokenKind::KwSuper
                | TokenKind::KwMove // for `move ||` closure
        )
    }
}

/// Helper trait for getting span from expressions.
trait ExprSpan {
    fn span(&self) -> Span;
}

impl ExprSpan for Expr {
    fn span(&self) -> Span {
        match self {
            Expr::Lit(_, s) => *s,
            Expr::Path(_, _, s) => *s,
            Expr::Block(_, s) => *s,
            Expr::Call { span, .. } => *span,
            Expr::MethodCall { span, .. } => *span,
            Expr::Field { span, .. } => *span,
            Expr::Index { span, .. } => *span,
            Expr::Unary { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
            Expr::Assign { span, .. } => *span,
            Expr::AddrOf { span, .. } => *span,
            Expr::Cast { span, .. } => *span,
            Expr::Try { span, .. } => *span,
            Expr::If { span, .. } => *span,
            Expr::Match { span, .. } => *span,
            Expr::Loop { span, .. } => *span,
            Expr::While { span, .. } => *span,
            Expr::For { span, .. } => *span,
            Expr::Closure { span, .. } => *span,
            Expr::Return { span, .. } => *span,
            Expr::Break { span, .. } => *span,
            Expr::Continue { span } => *span,
            Expr::Range { span, .. } => *span,
            Expr::Tuple { span, .. } => *span,
            Expr::Array { span, .. } => *span,
            Expr::Repeat { span, .. } => *span,
            Expr::Struct { span, .. } => *span,
            Expr::MacroCall { span, .. } => *span,
            Expr::Unsafe(_, s) => *s,
            Expr::Unit(s) => *s,
        }
    }
}
