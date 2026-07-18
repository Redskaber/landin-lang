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
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token>, interner: &'a Rodeo) -> Self {
        Self {
            tokens,
            pos: 0,
            interner,
            errors: Vec::new(),
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
        &self
            .tokens
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
        match &self.tokens[self.pos].kind {
            TokenKind::Ident(sym) | TokenKind::RawIdent(sym) => {
                Ident::new(*sym, self.current_span())
            }
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
        let vis = self.parse_visibility();
        let attrs = Vec::new(); // TODO: parse attributes

        let kind = match self.peek() {
            TokenKind::KwFn => ItemKind::Fn(self.parse_fn()),
            TokenKind::KwConst => ItemKind::Const(self.parse_const()),
            TokenKind::KwStatic => ItemKind::Static(self.parse_static()),
            TokenKind::KwStruct => ItemKind::Struct(self.parse_struct()),
            TokenKind::KwEnum => ItemKind::Enum(self.parse_enum()),
            TokenKind::KwTrait => ItemKind::Trait(self.parse_trait()),
            TokenKind::KwImpl => ItemKind::Impl(self.parse_impl()),
            TokenKind::KwType => ItemKind::TypeAlias(self.parse_type_alias()),
            TokenKind::KwExtern => ItemKind::ExternBlock(self.parse_extern_block()),
            TokenKind::KwMod => ItemKind::Mod(self.parse_mod()),
            TokenKind::KwUse => ItemKind::Use(self.parse_use()),
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

    fn parse_visibility(&mut self) -> Visibility {
        if *self.peek() == TokenKind::KwPub {
            self.bump();
            // TODO: parse pub(crate), pub(super), etc.
            if *self.peek() == TokenKind::LParen {
                self.bump();
                // Skip until )
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    self.bump();
                }
                self.eat(&TokenKind::RParen);
            }
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    fn parse_fn(&mut self) -> FnDecl {
        self.bump(); // fn
        let name = self.expect_ident("function name");
        let _ = name; // name is captured into FnDecl later (TODO: thread it through)
        let generics = self.parse_generics();
        self.expect(&TokenKind::LParen, "`(`");
        let inputs = self.parse_params();
        self.expect(&TokenKind::RParen, "`)`");
        let output = self.parse_return_type();
        let where_clause = self.parse_where_clause();
        let generics = Generics {
            params: generics,
            where_clause,
            span: Span::DUMMY,
        };

        let body = if *self.peek() == TokenKind::LBrace {
            Some(self.parse_block())
        } else {
            self.expect(&TokenKind::Semicolon, "`{` or `;`");
            None
        };

        FnDecl {
            sig: FnSig {
                inputs,
                output,
                abi: Abi::Landin,
                is_unsafe: false,
                span: Span::DUMMY,
            },
            body,
            generics,
        }
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
            // self parameter: self, &self, &mut self
            if *self.peek() == TokenKind::KwSelf_
                || (*self.peek() == TokenKind::And
                    && matches!(self.peek_at(1), TokenKind::KwSelf_ | TokenKind::KwMut))
            {
                let span = self.current_span();
                // Handle &self / &mut self
                if *self.peek() == TokenKind::And {
                    self.bump(); // &
                    if *self.peek() == TokenKind::KwMut {
                        self.bump();
                    }
                }
                self.bump(); // self
                             // Could be &self, &mut self, self, self: Type
                let pat = Pat::Ident(
                    BindingMode::ByValue,
                    Ident::new(Spur::default(), span),
                    None,
                );
                params.push(Param {
                    pat,
                    ty: Ty::Path(
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
                    ),
                    attrs: Vec::new(),
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
            if *self.peek() == TokenKind::Lifetime(Spur::default()) {
                // Lifetime param
                let span = self.current_span();
                let ident = self.ident_from_token(); // lifetime ident
                self.bump();
                params.push(GenericParam::Lifetime(LifetimeParam {
                    ident,
                    bounds: Vec::new(),
                    span,
                }));
            } else {
                // Type param
                let span = self.current_span();
                let ident = self.ident_from_token();
                self.bump();
                // Skip bounds for now
                while !matches!(
                    self.peek(),
                    TokenKind::Comma | TokenKind::Gt | TokenKind::Shr | TokenKind::Eof
                ) {
                    self.bump();
                }
                params.push(GenericParam::Type(TypeParam {
                    ident,
                    bounds: Vec::new(),
                    default: None,
                    span,
                }));
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        // Handle >> (two >) in type context
        if *self.peek() == TokenKind::Shr {
            // Split into two > — just eat one
            self.bump();
        } else {
            self.eat(&TokenKind::Gt);
        }
        params
    }

    fn parse_where_clause(&mut self) -> Vec<WherePredicate> {
        if *self.peek() != TokenKind::KwWhere {
            return Vec::new();
        }
        self.bump(); // where
        let preds = Vec::new();
        while !matches!(
            self.peek(),
            TokenKind::LBrace | TokenKind::Semicolon | TokenKind::Eof
        ) {
            // Skip predicate for now (simplified)
            while !matches!(
                self.peek(),
                TokenKind::Comma | TokenKind::LBrace | TokenKind::Semicolon | TokenKind::Eof
            ) {
                self.bump();
            }
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
        let _where = self.parse_where_clause();

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
                where_clause: Vec::new(),
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
        let _where = self.parse_where_clause();
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
                where_clause: Vec::new(),
                span: Span::DUMMY,
            },
            variants,
            span: Span::DUMMY,
        }
    }

    fn parse_trait(&mut self) -> TraitDecl {
        self.bump(); // trait
        let ident = self.ident_from_token();
        self.bump();
        let generics = self.parse_generics();
        // Skip supertraits for now
        while !matches!(self.peek(), TokenKind::LBrace | TokenKind::Eof) {
            self.bump();
        }
        self.expect(&TokenKind::LBrace, "`{`");
        let items = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            // Simplified: skip trait items
            self.bump();
        }
        self.expect(&TokenKind::RBrace, "`}`");

        TraitDecl {
            ident,
            generics: Generics {
                params: generics,
                where_clause: Vec::new(),
                span: Span::DUMMY,
            },
            supertraits: Vec::new(),
            items,
            span: Span::DUMMY,
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
        let _where = self.parse_where_clause();
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
                where_clause: Vec::new(),
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
        self.expect(&TokenKind::Eq, "`=`");
        let ty = self.parse_ty();
        self.expect(&TokenKind::Semicolon, "`;`");
        TypeAliasDecl {
            ident,
            generics: Generics {
                params: generics,
                where_clause: Vec::new(),
                span: Span::DUMMY,
            },
            ty,
            span: Span::DUMMY,
        }
    }

    fn parse_extern_block(&mut self) -> ExternBlock {
        self.bump(); // extern
        let abi = {
            if let TokenKind::StrLit(sym) = self.peek().clone() {
                self.bump();
                let s = self.interner.resolve(&sym);
                match s {
                    "C" => Abi::C,
                    "System" => Abi::System,
                    _ => Abi::Landin,
                }
            } else {
                Abi::Landin
            }
        };
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
            span: Span::DUMMY,
        }
    }

    fn parse_mod(&mut self) -> ModDecl {
        self.bump(); // mod
        let _ident = self.ident_from_token();
        self.bump();
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
            ModDecl::Inline(items, Span::DUMMY)
        } else {
            self.expect(&TokenKind::Semicolon, "`;`");
            ModDecl::Loaded(Span::DUMMY)
        }
    }

    fn parse_use(&mut self) -> UseDecl {
        self.bump(); // use
        let path = self.parse_path();
        // Simplified: just leaf
        self.expect(&TokenKind::Semicolon, "`;`");
        UseDecl {
            tree: UseTree::Leaf(path, None),
            span: Span::DUMMY,
        }
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
                let _lifetime: Option<Lifetime> = if let TokenKind::Lifetime(_) = self.peek() {
                    self.bump();
                    None
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
                Ty::Ref(None, mutability, Box::new(ty), span)
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
                self.expect(&TokenKind::LParen, "`(`");
                let inputs = Vec::new();
                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                    // Skip param patterns, just get types (simplified)
                    while !matches!(
                        self.peek(),
                        TokenKind::Comma | TokenKind::RParen | TokenKind::Eof
                    ) {
                        self.bump();
                    }
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
            TokenKind::Underscore => {
                self.bump();
                Ty::Infer(span)
            }
            _ => {
                // Path type
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
            _ => PathLeading::None,
        };

        // Ensure the first token is an identifier
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
        segments.push(PathSegment { ident, args: None });

        while *self.peek() == TokenKind::PathSep {
            self.bump();
            let ident = self.ident_from_token();
            self.bump();
            // Skip generic args for now
            segments.push(PathSegment { ident, args: None });
        }

        Path {
            segments,
            leading,
            span,
        }
    }

    // --- Patterns (simplified) ---

    fn parse_pat(&mut self) -> Pat {
        let span = self.current_span();
        match self.peek() {
            TokenKind::Underscore => {
                self.bump();
                Pat::Wild(span)
            }
            TokenKind::KwMut => {
                self.bump();
                let ident = self.ident_from_token();
                self.bump();
                Pat::Ident(BindingMode::ByValue, ident, None)
            }
            TokenKind::And => {
                self.bump();
                let mutability = if *self.peek() == TokenKind::KwMut {
                    self.bump();
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                let pat = self.parse_pat();
                Pat::Ref(Box::new(pat), mutability, span)
            }
            _ => {
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
            // Check for let statement
            if *self.peek() == TokenKind::KwLet {
                let stmt = self.parse_let();
                stmts.push(Stmt::Local(stmt));
                continue;
            }

            // Parse expression
            let expr = self.parse_expr();
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
                let cond = self.parse_expr();
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
                let iter = self.parse_expr();
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
            TokenKind::Or | TokenKind::OrOr => {
                // Closure: |args| expr or || expr (empty params)
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
            _ => {
                // Path expression (struct literal deferred to month 3)
                let path = self.parse_path();
                Expr::Path(None, path, span)
            }
        }
    }

    fn parse_if_expr(&mut self) -> Expr {
        let span = self.current_span();
        self.bump(); // if
        let cond = self.parse_expr();
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
        let expr = self.parse_expr();
        self.expect(&TokenKind::LBrace, "`{`");
        let mut arms = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let pat = self.parse_pat();
            let guard = if *self.peek() == TokenKind::KwIf {
                self.bump();
                Some(self.parse_expr())
            } else {
                None
            };
            self.expect(&TokenKind::FatArrow, "`=>`");
            let body = self.parse_expr();
            self.eat(&TokenKind::Comma);
            arms.push(Arm {
                pat,
                guard,
                body: Box::new(body),
                span: Span::DUMMY,
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
            Expr::Unsafe(_, s) => *s,
            Expr::Unit(s) => *s,
        }
    }
}
