//! Stage 6.12 (TD-022): Item-level parsing sub-module.
//!
//! Per 02-grammar.md §3.1 (Crate + module + item) + §3.7 (use declaration).
//! Extracted from `parser.rs` per `docs/stage-committee-process.md` v3.21
//! §14.4 + §13.4.
//!
//! Owns 16 item-parsing functions:
//! - `parse_item` (dispatcher)
//! - `parse_outer_attrs` / `parse_attr_args` / `parse_visibility`
//! - `parse_fn` / `parse_const` / `parse_static`
//! - `parse_struct` / `parse_enum`
//! - `parse_trait` / `parse_impl` / `parse_type_alias`
//! - `parse_extern_block_or_fn` / `parse_mod`
//! - `parse_use` / `parse_use_tree`
//!
//! Also includes the file-level `ty_to_path` helper used by `parse_impl`.

use crate::ast::*;
use crate::lexer::token::*;

use super::parser::Parser;

impl<'a> Parser<'a> {
    pub(super) fn parse_item(&mut self) -> Option<Item> {
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
            TokenKind::KwTrait => ItemKind::Trait(self.parse_trait(false)),
            TokenKind::KwImpl => ItemKind::Impl(self.parse_impl(false)),
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
            // Stage 3.65: now propagates `is_unsafe` to the AST `ImplDecl`
            // (previously dropped — Stage 1.0 debt).
            TokenKind::KwUnsafe if matches!(self.peek_at(1), TokenKind::KwImpl) => {
                self.bump(); // consume `unsafe`
                ItemKind::Impl(self.parse_impl(true))
            }
            // unsafe trait — `unsafe trait Foo { ... }`
            // Stage 3.65: now propagates `is_unsafe` to the AST `TraitDecl`
            // (previously dropped — Stage 1.0 debt).
            TokenKind::KwUnsafe if matches!(self.peek_at(1), TokenKind::KwTrait) => {
                self.bump(); // consume `unsafe`
                ItemKind::Trait(self.parse_trait(true))
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
    pub(super) fn parse_outer_attrs(&mut self) -> Vec<Attr> {
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
    pub(super) fn parse_attr_args(&mut self) -> AttrArgs {
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

    pub(super) fn parse_visibility(&mut self) -> Visibility {
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

    pub(super) fn parse_fn(&mut self, is_unsafe: bool, abi: Abi) -> FnDecl {
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

    pub(super) fn parse_const(&mut self) -> ConstDecl {
        let kw_span = self.current_span(); // Stage 3.67: capture `const` keyword span
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
            span: kw_span,
        }
    }

    pub(super) fn parse_static(&mut self) -> StaticDecl {
        let kw_span = self.current_span(); // Stage 3.67: capture `static` keyword span
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
            span: kw_span,
        }
    }

    pub(super) fn parse_struct(&mut self) -> StructDecl {
        let kw_span = self.current_span(); // Stage 3.67: capture `struct` keyword span
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
                span: kw_span,
            },
            fields,
            is_unit,
            is_tuple,
            span: kw_span,
        }
    }

    pub(super) fn parse_enum(&mut self) -> EnumDecl {
        let kw_span = self.current_span(); // Stage 3.67: capture `enum` keyword span
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
                span: kw_span,
            },
            variants,
            span: kw_span,
        }
    }

    pub(super) fn parse_trait(&mut self, is_unsafe: bool) -> TraitDecl {
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
            is_unsafe,
            span: self.current_span(),
        }
    }

    pub(super) fn parse_impl(&mut self, is_unsafe: bool) -> ImplDecl {
        let kw_span = self.current_span(); // Stage 3.67: capture `impl` keyword span
        self.bump(); // impl
        let generics = self.parse_generics();

        // Stage 5.5 audit fix: correctly distinguish self_ty from of_trait.
        //
        // Grammar (per docs/lang-design/02-grammar.md §174):
        //   impl generic_params? type "for" type where_clause? "{" impl_item* "}"
        //   impl generic_params? type where_clause? "{" impl_item* "}"   (inherent impl)
        //
        // Semantics (matches Rust): `impl Trait for SelfType` — the FIRST
        // type is the trait, the SECOND type (after `for`) is the self type.
        // For inherent impls (`impl SelfType { ... }`), there is only one
        // type and it is the self type.
        //
        // BUG (fixed here): previously the parser unconditionally assigned
        // the first parsed type to `self_ty` and the second (after `for`)
        // to `of_trait`, which is backwards for trait impls. This caused
        // TraitResolver to build vtables with swapped keys and broke
        // `find_vtable(trait, self_ty)` lookups (test_vtable_query failure).
        //
        // Fix: peek ahead. If `for` follows the first type, the first type
        // is the trait; otherwise it is the self type (inherent impl).
        let first_ty = self.parse_ty();
        let (of_trait, self_ty) = if *self.peek() == TokenKind::KwFor {
            // `impl <first_ty=Trait> for <self_ty> { ... }`
            // first_ty is a path to the trait — convert it to a Path.
            // parse_ty returns a Ty::Path for simple paths; extract it.
            let trait_path = ty_to_path(first_ty);
            self.bump(); // for
            let self_ty = self.parse_ty();
            (Some(trait_path), self_ty)
        } else {
            // `impl <self_ty> { ... }` (inherent impl)
            (None, first_ty)
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
                span: kw_span,
            },
            of_trait,
            self_ty,
            items,
            is_unsafe,
            span: kw_span,
        }
    }

    pub(super) fn parse_type_alias(&mut self) -> TypeAliasDecl {
        let kw_span = self.current_span(); // Stage 3.67: capture `type` keyword span
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
                span: kw_span,
            },
            ty,
            span: kw_span,
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
    pub(super) fn parse_extern_block_or_fn(&mut self) -> ExternBlock {
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

    pub(super) fn parse_mod(&mut self) -> ModDecl {
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

    pub(super) fn parse_use(&mut self) -> UseDecl {
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
    pub(super) fn parse_use_tree(&mut self) -> UseTree {
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
}

/// Stage 5.5 audit fix: extract a `Path` from a `Ty::Path`.
///
/// Used by `parse_impl` to convert the first parsed type (the trait path)
/// into a `Path` for `ImplDecl.of_trait`. For non-path types (e.g. `impl
/// (i32) for Foo` — which is invalid syntax), returns a dummy path; the
/// caller's `expect(LBrace)` will then fail with a clear error.
fn ty_to_path(ty: Ty) -> Path {
    match ty {
        Ty::Path(_qself, path, _span) => path,
        // Non-path types in trait position are invalid; return a dummy so
        // parsing continues and produces a parse error at the expected `{`.
        other => Path {
            segments: vec![],
            leading: PathLeading::None,
            span: match &other {
                Ty::Bool(s)
                | Ty::Char(s)
                | Ty::Never(s)
                | Ty::Infer(s)
                | Ty::Slice(_, s)
                | Ty::Array(_, _, s)
                | Ty::Ref(_, _, _, s)
                | Ty::Ptr(_, _, s) => *s,
                Ty::Int(_, s) | Ty::Uint(_, s) | Ty::Float(_, s) => *s,
                Ty::Tuple(_, s) => *s,
                Ty::FnPtr { span: s, .. } => *s,
                Ty::TraitObject { span: s, .. } => *s,
                Ty::ImplTrait(_, s) => *s,
                Ty::Path(_, _, s) => *s,
            },
        },
    }
}
