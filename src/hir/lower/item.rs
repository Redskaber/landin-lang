//! Item lowering: AST items → HIR items.

use crate::ast::{self, Visibility};
use crate::hir::kinds::*;
use crate::hir::lower::body;
use crate::hir::lower::cx::HirLowerCtxt;
use crate::hir::lower::generics;
use crate::hir::lower::ty;
use crate::session::Span;

impl<'a> HirLowerCtxt<'a> {
    /// Lower an AST item to a HIR owner node and store it.
    pub fn lower_item(&mut self, ast_item: &ast::Item) {
        let def_id = self.enter_owner();
        let hir_id = self.owner_hir_id();
        let vis = ast_item.vis.clone();
        let attrs = ast_item.attrs.clone();
        let span = ast_item.span;

        let hir_item = match &ast_item.kind {
            ast::ItemKind::Fn(fn_decl) => {
                let hir_fn = self.lower_fn(fn_decl, hir_id, vis.clone(), attrs.clone(), span);
                HirItem::Fn(hir_fn)
            }
            ast::ItemKind::Const(c) => {
                let hir_c = self.lower_const(c, hir_id, vis.clone(), attrs.clone(), span);
                HirItem::Const(hir_c)
            }
            ast::ItemKind::Static(s) => {
                let hir_s = self.lower_static(s, hir_id, vis.clone(), attrs.clone(), span);
                HirItem::Static(hir_s)
            }
            ast::ItemKind::Struct(s) => {
                let hir_s = self.lower_struct(s, hir_id, vis.clone(), attrs.clone(), span);
                HirItem::Struct(hir_s)
            }
            ast::ItemKind::Enum(e) => {
                let hir_e = self.lower_enum(e, hir_id, vis.clone(), attrs.clone(), span);
                HirItem::Enum(hir_e)
            }
            ast::ItemKind::Trait(t) => {
                let hir_t = self.lower_trait(t, hir_id, vis.clone(), attrs.clone(), span);
                HirItem::Trait(hir_t)
            }
            ast::ItemKind::Impl(i) => {
                let hir_i = self.lower_impl(i, hir_id, attrs.clone(), span);
                HirItem::Impl(hir_i)
            }
            ast::ItemKind::TypeAlias(t) => {
                let hir_t = self.lower_type_alias(t, hir_id, vis.clone(), attrs.clone(), span);
                HirItem::TypeAlias(hir_t)
            }
            ast::ItemKind::ExternBlock(eb) => {
                let hir_eb = self.lower_extern_block(eb, hir_id, attrs.clone(), span);
                HirItem::ExternBlock(hir_eb)
            }
            ast::ItemKind::Mod(m) => {
                let hir_m = self.lower_mod(m, hir_id, vis.clone(), attrs.clone(), span);
                HirItem::Mod(hir_m)
            }
            ast::ItemKind::Use(u) => {
                let hir_u = self.lower_use(u, hir_id, vis.clone(), attrs.clone(), span);
                HirItem::Use(hir_u)
            }
            // Stage 18.02: macro_rules! — store as no-op for now (Phase 2 will expand).
            ast::ItemKind::MacroRules(_) => {
                // Macro definitions are not lowered to HIR items — they're
                // expanded before HIR lowering in Phase 2.
                // For now, return early without storing an owner.
                self.exit_owner();
                return;
            }
        };

        self.store_owner(def_id, OwnerNode::Item(hir_item));
        self.exit_owner();
    }

    fn lower_fn(
        &mut self,
        fn_decl: &ast::FnDecl,
        hir_id: HirId,
        vis: Visibility,
        attrs: Vec<ast::Attr>,
        span: Span,
    ) -> HirFn {
        let generics = generics::lower_generics(self, &fn_decl.generics);
        let inputs: Vec<HirParam> = fn_decl
            .sig
            .inputs
            .iter()
            .map(|p| self.lower_param(p))
            .collect();
        let output = match &fn_decl.sig.output {
            ast::FnRetTy::Default(s) => HirFnRetTy::Default(*s),
            ast::FnRetTy::Ty(t) => HirFnRetTy::Ty(ty::lower_ty(self, t)),
        };
        let body = if let Some(b) = &fn_decl.body {
            let hir_params = inputs.clone();
            let body = body::lower_body(self, b, hir_params);
            Some(self.store_body(body))
        } else {
            None
        };
        HirFn {
            hir_id,
            ident: fn_decl.ident,
            generics,
            sig: HirFnSig {
                inputs,
                output,
                abi: fn_decl.sig.abi,
                is_unsafe: fn_decl.sig.is_unsafe,
                span: fn_decl.sig.span,
            },
            body,
            vis,
            attrs,
            span,
        }
    }

    pub fn lower_param(&mut self, p: &ast::Param) -> HirParam {
        // For self params with shorthand (no explicit type), skip lowering
        // the placeholder ty. For all other params, lower the ty.
        let hir_ty = if p.is_self && p.self_kind.is_some() {
            // Check if ty is the placeholder (Path with empty Spur segment)
            if let ast::Ty::Path(_, path, _) = &p.ty {
                if path.segments.len() == 1 && path.segments[0].ident.name == lasso::Spur::default()
                {
                    None
                } else {
                    Some(ty::lower_ty(self, &p.ty))
                }
            } else {
                Some(ty::lower_ty(self, &p.ty))
            }
        } else {
            Some(ty::lower_ty(self, &p.ty))
        };
        HirParam {
            hir_id: self.fresh_hir_id(),
            pat: crate::hir::lower::pat::lower_pat(self, &p.pat),
            ty: hir_ty,
            self_kind: p.self_kind,
            span: p.span,
        }
    }

    fn lower_const(
        &mut self,
        c: &ast::ConstDecl,
        hir_id: HirId,
        vis: Visibility,
        attrs: Vec<ast::Attr>,
        span: Span,
    ) -> HirConst {
        let ty = ty::lower_ty(self, &c.ty);
        let body = body::lower_body_from_expr(self, &c.expr, vec![]);
        HirConst {
            hir_id,
            ident: c.ident,
            ty,
            body,
            vis,
            attrs,
            span,
        }
    }

    fn lower_static(
        &mut self,
        s: &ast::StaticDecl,
        hir_id: HirId,
        vis: Visibility,
        attrs: Vec<ast::Attr>,
        span: Span,
    ) -> HirStatic {
        let ty = ty::lower_ty(self, &s.ty);
        let body = body::lower_body_from_expr(self, &s.expr, vec![]);
        HirStatic {
            hir_id,
            ident: s.ident,
            ty,
            mutability: if s.is_mut {
                crate::ast::Mutability::Mutable
            } else {
                crate::ast::Mutability::Immutable
            },
            body,
            vis,
            attrs,
            span,
        }
    }

    fn lower_struct(
        &mut self,
        s: &ast::StructDecl,
        hir_id: HirId,
        vis: Visibility,
        attrs: Vec<ast::Attr>,
        span: Span,
    ) -> HirStruct {
        let generics = generics::lower_generics(self, &s.generics);
        let fields: Vec<HirFieldDef> = s
            .fields
            .iter()
            .map(|f| HirFieldDef {
                hir_id: self.fresh_hir_id(),
                vis: f.vis.clone(),
                ident: f.ident,
                ty: ty::lower_ty(self, &f.ty),
                attrs: vec![],
                span: f.span,
            })
            .collect();
        HirStruct {
            hir_id,
            ident: s.ident,
            generics,
            fields,
            is_unit: s.is_unit,
            is_tuple: s.is_tuple,
            vis,
            attrs,
            span,
        }
    }

    fn lower_enum(
        &mut self,
        e: &ast::EnumDecl,
        hir_id: HirId,
        vis: Visibility,
        attrs: Vec<ast::Attr>,
        span: Span,
    ) -> HirEnum {
        let generics = generics::lower_generics(self, &e.generics);
        let variants: Vec<HirVariant> = e
            .variants
            .iter()
            .map(|v| {
                let data = match &v.data {
                    ast::VariantData::Unit(s) => HirVariantData::Unit(*s),
                    ast::VariantData::Tuple(fields, s) => {
                        let hir_fields: Vec<HirFieldDef> = fields
                            .iter()
                            .map(|f| HirFieldDef {
                                hir_id: self.fresh_hir_id(),
                                vis: f.vis.clone(),
                                ident: f.ident,
                                ty: ty::lower_ty(self, &f.ty),
                                attrs: vec![],
                                span: f.span,
                            })
                            .collect();
                        HirVariantData::Tuple(hir_fields, *s)
                    }
                    ast::VariantData::Struct(fields, s) => {
                        let hir_fields: Vec<HirFieldDef> = fields
                            .iter()
                            .map(|f| HirFieldDef {
                                hir_id: self.fresh_hir_id(),
                                vis: f.vis.clone(),
                                ident: f.ident,
                                ty: ty::lower_ty(self, &f.ty),
                                attrs: vec![],
                                span: f.span,
                            })
                            .collect();
                        HirVariantData::Struct(hir_fields, *s)
                    }
                };
                HirVariant {
                    hir_id: self.fresh_hir_id(),
                    ident: v.ident,
                    data,
                    attrs: vec![],
                    span: v.span,
                }
            })
            .collect();
        HirEnum {
            hir_id,
            ident: e.ident,
            generics,
            variants,
            vis,
            attrs,
            span,
        }
    }

    fn lower_trait(
        &mut self,
        t: &ast::TraitDecl,
        hir_id: HirId,
        vis: Visibility,
        attrs: Vec<ast::Attr>,
        span: Span,
    ) -> HirTrait {
        let generics = generics::lower_generics(self, &t.generics);
        let supertraits: Vec<HirTypeBound> = generics::lower_type_bounds(self, &t.supertraits);
        let items: Vec<HirTraitItem> = t.items.iter().map(|ti| self.lower_trait_item(ti)).collect();
        HirTrait {
            hir_id,
            ident: t.ident,
            generics,
            supertraits,
            items,
            vis,
            attrs,
            is_unsafe: t.is_unsafe,
            span,
        }
    }

    fn lower_trait_item(&mut self, ti: &ast::TraitItem) -> HirTraitItem {
        match ti {
            ast::TraitItem::Fn(ident, generics, sig, body) => {
                // Stage 14.97 (Bug Y1 fix): For trait methods WITH a body (default
                // implementations), enter a new owner context so each method
                // gets its own DefId. This is required for:
                //   - fn_name_by_def_id registration (otherwise all trait methods
                //     in the same trait share the trait's DefId, causing name
                //     collisions like landin_Counter_default_doubled_increment
                //     for both increment and doubled_increment)
                //   - fn_sig_table registration (same reason)
                //   - vtable / call site resolution (callers look up by DefId)
                //
                // For trait methods WITHOUT bodies (just declarations), we keep
                // the old behavior (no separate owner) — they don't need to be
                // codegen'd as functions, so they don't need their own DefId.
                //
                // Per §1.0 原则 6 "通用 > 特例": when bodies are present, use the
                // same enter_owner/exit_owner pattern as lower_impl.
                if body.is_some() {
                    let def_id = self.enter_owner();
                    let fn_hir_id = self.owner_hir_id();
                    let hir_generics = generics::lower_generics(self, generics);
                    let inputs: Vec<HirParam> =
                        sig.inputs.iter().map(|p| self.lower_param(p)).collect();
                    let output = match &sig.output {
                        ast::FnRetTy::Default(s) => HirFnRetTy::Default(*s),
                        ast::FnRetTy::Ty(t) => HirFnRetTy::Ty(ty::lower_ty(self, t)),
                    };
                    let hir_body = if let Some(b) = body {
                        let hir_body = body::lower_body(self, b, inputs.clone());
                        Some(self.store_body(hir_body))
                    } else {
                        None
                    };
                    let hir_fn = HirFn {
                        hir_id: fn_hir_id,
                        ident: *ident,
                        generics: hir_generics,
                        sig: HirFnSig {
                            inputs,
                            output,
                            abi: sig.abi,
                            is_unsafe: sig.is_unsafe,
                            span: sig.span,
                        },
                        body: hir_body,
                        vis: Visibility::Private,
                        attrs: vec![],
                        span: sig.span,
                    };
                    self.store_owner(def_id, OwnerNode::Item(HirItem::Fn(hir_fn.clone())));
                    self.exit_owner();
                    HirTraitItem::Fn(hir_fn)
                } else {
                    // No body — keep old behavior (no separate owner).
                    let hir_generics = generics::lower_generics(self, generics);
                    let inputs: Vec<HirParam> =
                        sig.inputs.iter().map(|p| self.lower_param(p)).collect();
                    let output = match &sig.output {
                        ast::FnRetTy::Default(s) => HirFnRetTy::Default(*s),
                        ast::FnRetTy::Ty(t) => HirFnRetTy::Ty(ty::lower_ty(self, t)),
                    };
                    let fn_hir_id = self.fresh_hir_id();
                    let hir_fn = HirFn {
                        hir_id: fn_hir_id,
                        ident: *ident,
                        generics: hir_generics,
                        sig: HirFnSig {
                            inputs,
                            output,
                            abi: sig.abi,
                            is_unsafe: sig.is_unsafe,
                            span: sig.span,
                        },
                        body: None,
                        vis: Visibility::Private,
                        attrs: vec![],
                        span: sig.span,
                    };
                    HirTraitItem::Fn(hir_fn)
                }
            }
            ast::TraitItem::Type(ident, generics, bounds, default) => {
                // Stage 18.52 GATs Phase 1: lower generics field for GAT support.
                // Per §1.0 原則 6 "通用 > 特例": reuse existing `lower_generics`.
                let hir_generics = generics::lower_generics(self, generics);
                let hir_bounds = generics::lower_type_bounds(self, bounds);
                let hir_default = default.as_ref().map(|t| ty::lower_ty(self, t));
                HirTraitItem::Type(HirAssocType {
                    hir_id: self.fresh_hir_id(),
                    ident: *ident,
                    generics: hir_generics,
                    bounds: hir_bounds,
                    default: hir_default,
                    span: Span::DUMMY,
                })
            }
            ast::TraitItem::Const(ident, ty, default) => {
                let hir_ty = ty::lower_ty(self, ty);
                let hir_default = default
                    .as_ref()
                    .map(|e| body::lower_body_from_expr(self, e, vec![]));
                HirTraitItem::Const(HirAssocConst {
                    hir_id: self.fresh_hir_id(),
                    ident: *ident,
                    ty: hir_ty,
                    default: hir_default,
                    span: Span::DUMMY,
                })
            }
        }
    }

    fn lower_impl(
        &mut self,
        i: &ast::ImplDecl,
        hir_id: HirId,
        attrs: Vec<ast::Attr>,
        span: Span,
    ) -> HirImpl {
        let generics = generics::lower_generics(self, &i.generics);
        let self_ty = ty::lower_ty(self, &i.self_ty);
        let of_trait = i
            .of_trait
            .as_ref()
            .map(|p| crate::hir::lower::path::lower_path(self, p));
        let items: Vec<HirImplItem> = i
            .items
            .iter()
            .filter_map(|item| {
                // Lower impl items as nested owners.
                if let ast::ItemKind::Fn(fn_decl) = &item.kind {
                    let def_id = self.enter_owner();
                    let fn_hir_id = self.owner_hir_id();
                    let hir_fn = self.lower_fn(
                        fn_decl,
                        fn_hir_id,
                        item.vis.clone(),
                        item.attrs.clone(),
                        item.span,
                    );
                    self.store_owner(def_id, OwnerNode::Item(HirItem::Fn(hir_fn.clone())));
                    self.exit_owner();
                    Some(HirImplItem::Fn(hir_fn))
                } else if let ast::ItemKind::Const(c) = &item.kind {
                    let def_id = self.enter_owner();
                    let c_hir_id = self.owner_hir_id();
                    let hir_c = self.lower_const(
                        c,
                        c_hir_id,
                        item.vis.clone(),
                        item.attrs.clone(),
                        item.span,
                    );
                    self.store_owner(def_id, OwnerNode::Item(HirItem::Const(hir_c.clone())));
                    self.exit_owner();
                    Some(HirImplItem::Const(hir_c))
                } else if let ast::ItemKind::TypeAlias(t) = &item.kind {
                    let def_id = self.enter_owner();
                    let t_hir_id = self.owner_hir_id();
                    let hir_t = self.lower_type_alias(
                        t,
                        t_hir_id,
                        item.vis.clone(),
                        item.attrs.clone(),
                        item.span,
                    );
                    self.store_owner(def_id, OwnerNode::Item(HirItem::TypeAlias(hir_t.clone())));
                    self.exit_owner();
                    Some(HirImplItem::Type(HirAssocType {
                        hir_id: hir_t.hir_id,
                        ident: hir_t.ident,
                        // Stage 18.52 GATs Phase 1: preserve generics from TypeAliasDecl.
                        // Previously this branch silently discarded `hir_t.generics`,
                        // making `impl Trait { type Item<'a> = &'a T; }` lose the
                        // lifetime parameter. Now we propagate it for GAT support.
                        generics: hir_t.generics.clone(),
                        bounds: vec![],
                        default: Some(hir_t.ty),
                        span: hir_t.span,
                    }))
                } else {
                    None
                }
            })
            .collect();
        HirImpl {
            hir_id,
            generics,
            of_trait,
            self_ty,
            items,
            attrs,
            is_unsafe: i.is_unsafe,
            span,
        }
    }

    fn lower_type_alias(
        &mut self,
        t: &ast::TypeAliasDecl,
        hir_id: HirId,
        vis: Visibility,
        attrs: Vec<ast::Attr>,
        span: Span,
    ) -> HirTypeAlias {
        let generics = generics::lower_generics(self, &t.generics);
        let ty = ty::lower_ty(self, &t.ty);
        HirTypeAlias {
            hir_id,
            ident: t.ident,
            generics,
            ty,
            vis,
            attrs,
            span,
        }
    }

    fn lower_extern_block(
        &mut self,
        eb: &ast::ExternBlock,
        hir_id: HirId,
        attrs: Vec<ast::Attr>,
        span: Span,
    ) -> HirExternBlock {
        let items: Vec<HirForeignItem> = eb
            .items
            .iter()
            .filter_map(|item| {
                let def_id = self.enter_owner();
                let item_hir_id = self.owner_hir_id();
                let hir_item = match &item.kind {
                    ast::ItemKind::Fn(fn_decl) => {
                        let hir_fn = self.lower_fn(
                            fn_decl,
                            item_hir_id,
                            item.vis.clone(),
                            item.attrs.clone(),
                            item.span,
                        );
                        HirForeignItem::Fn(hir_fn)
                    }
                    ast::ItemKind::Static(s) => {
                        let hir_s = self.lower_static(
                            s,
                            item_hir_id,
                            item.vis.clone(),
                            item.attrs.clone(),
                            item.span,
                        );
                        HirForeignItem::Static(hir_s)
                    }
                    ast::ItemKind::TypeAlias(t) => {
                        let hir_t = self.lower_type_alias(
                            t,
                            item_hir_id,
                            item.vis.clone(),
                            item.attrs.clone(),
                            item.span,
                        );
                        HirForeignItem::TypeAlias(hir_t)
                    }
                    _ => {
                        self.exit_owner();
                        return None;
                    }
                };
                self.store_owner(
                    def_id,
                    OwnerNode::Item(match hir_item.clone() {
                        HirForeignItem::Fn(f) => HirItem::Fn(f),
                        HirForeignItem::Static(s) => HirItem::Static(s),
                        HirForeignItem::TypeAlias(t) => HirItem::TypeAlias(t),
                    }),
                );
                self.exit_owner();
                Some(hir_item)
            })
            .collect();
        HirExternBlock {
            hir_id,
            abi: eb.abi,
            items,
            attrs,
            span,
        }
    }

    fn lower_mod(
        &mut self,
        m: &ast::ModDecl,
        hir_id: HirId,
        vis: Visibility,
        attrs: Vec<ast::Attr>,
        _span: Span,
    ) -> HirMod {
        let (ident, mod_kind, mod_span) = match m {
            ast::ModDecl::Inline { ident, items, span } => {
                let hir_items: Vec<HirItem> = items
                    .iter()
                    .map(|item| {
                        let def_id = self.enter_owner();
                        let item_hir_id = self.owner_hir_id();
                        let hir_item = match &item.kind {
                            ast::ItemKind::Fn(fn_decl) => HirItem::Fn(self.lower_fn(
                                fn_decl,
                                item_hir_id,
                                item.vis.clone(),
                                item.attrs.clone(),
                                item.span,
                            )),
                            ast::ItemKind::Const(c) => HirItem::Const(self.lower_const(
                                c,
                                item_hir_id,
                                item.vis.clone(),
                                item.attrs.clone(),
                                item.span,
                            )),
                            ast::ItemKind::Static(s) => HirItem::Static(self.lower_static(
                                s,
                                item_hir_id,
                                item.vis.clone(),
                                item.attrs.clone(),
                                item.span,
                            )),
                            ast::ItemKind::Struct(s) => HirItem::Struct(self.lower_struct(
                                s,
                                item_hir_id,
                                item.vis.clone(),
                                item.attrs.clone(),
                                item.span,
                            )),
                            ast::ItemKind::Enum(e) => HirItem::Enum(self.lower_enum(
                                e,
                                item_hir_id,
                                item.vis.clone(),
                                item.attrs.clone(),
                                item.span,
                            )),
                            ast::ItemKind::Trait(t) => HirItem::Trait(self.lower_trait(
                                t,
                                item_hir_id,
                                item.vis.clone(),
                                item.attrs.clone(),
                                item.span,
                            )),
                            ast::ItemKind::Impl(i) => HirItem::Impl(self.lower_impl(
                                i,
                                item_hir_id,
                                item.attrs.clone(),
                                item.span,
                            )),
                            ast::ItemKind::TypeAlias(t) => {
                                HirItem::TypeAlias(self.lower_type_alias(
                                    t,
                                    item_hir_id,
                                    item.vis.clone(),
                                    item.attrs.clone(),
                                    item.span,
                                ))
                            }
                            ast::ItemKind::ExternBlock(eb) => {
                                HirItem::ExternBlock(self.lower_extern_block(
                                    eb,
                                    item_hir_id,
                                    item.attrs.clone(),
                                    item.span,
                                ))
                            }
                            ast::ItemKind::Mod(m) => HirItem::Mod(self.lower_mod(
                                m,
                                item_hir_id,
                                item.vis.clone(),
                                item.attrs.clone(),
                                item.span,
                            )),
                            ast::ItemKind::Use(u) => HirItem::Use(self.lower_use(
                                u,
                                item_hir_id,
                                item.vis.clone(),
                                item.attrs.clone(),
                                item.span,
                            )),
                            // Stage 18.02: macro_rules! — skip (no HIR lowering).
                            // Return a dummy Use item (will be filtered out).
                            ast::ItemKind::MacroRules(_) => {
                                self.exit_owner();
                                HirItem::Use(crate::hir::HirUse {
                                    hir_id: item_hir_id,
                                    tree: crate::hir::HirUseTree::Glob(crate::hir::HirPath {
                                        hir_id: item_hir_id,
                                        segments: vec![],
                                        leading: crate::ast::PathLeading::None,
                                        res: crate::hir::Res::Unknown,
                                        span: item.span,
                                    }),
                                    vis: item.vis.clone(),
                                    attrs: item.attrs.clone(),
                                    span: item.span,
                                })
                            }
                        };
                        self.store_owner(def_id, OwnerNode::Item(hir_item.clone()));
                        self.exit_owner();
                        hir_item
                    })
                    .collect();
                (*ident, HirModKind::Inline(hir_items), *span)
            }
            ast::ModDecl::Loaded { ident, span } => (*ident, HirModKind::Loaded, *span),
        };
        HirMod {
            hir_id,
            ident,
            kind: mod_kind,
            vis,
            attrs,
            span: mod_span,
        }
    }

    fn lower_use(
        &mut self,
        u: &ast::UseDecl,
        hir_id: HirId,
        vis: Visibility,
        attrs: Vec<ast::Attr>,
        span: Span,
    ) -> HirUse {
        let tree = generics::lower_use_tree(self, &u.tree);
        HirUse {
            hir_id,
            tree,
            vis,
            attrs,
            span,
        }
    }
}
