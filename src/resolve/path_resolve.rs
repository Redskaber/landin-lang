//! Stage 6.16 (TD-026): Late resolve — path/expr/body resolution.
//!
//! Per 01-language-specification.md §6.2 (resolve order) pass 4-5:
//! - pass 4: late resolve crate (resolve all path expressions, type paths,
//!   pattern paths)
//! - pass 5: resolve main (determine crate root)
//!
//! Extracted from `resolver.rs` per `docs/stage-committee-process.md`
//! v3.21 §14.4 + §13.4.
//!
//! Owns 11 functions:
//! - `resolve_all_paths` / `resolve_owner_paths` / `resolve_item_paths` (pass 4 dispatchers)
//! - `resolve_generics_paths` / `resolve_ty_paths` / `resolve_hir_path` /
//!   `resolve_path` (pass 4 type/path resolution)
//! - `resolve_body` / `collect_pat_bindings` / `resolve_expr` / `resolve_block`
//!   (pass 4 body resolution)

use crate::ast::PathLeading;
use crate::hir::*;
use crate::resolve::scope::{ScopeKind, ScopeStack};
use lasso::Rodeo;
use lasso::Spur;
use std::collections::HashMap;

use super::primitives::lookup_prim_ty;
use super::resolver::Resolver;

impl Resolver {
    // ================================================================
    // Phase 3: Resolve all HirPath nodes
    // ================================================================

    pub(super) fn resolve_all_paths(&mut self, hir: &mut HirCrate, interner: &Rodeo) {
        // Stage 3.67: Build a map from owner DefId → HirSelfKind so that
        // body resolution can know whether it's inside a trait or impl.
        // Previously (Stage 3.66), only owner-level paths got the
        // accurate HirSelfKind; body-level `Self` always defaulted to
        // Impl. Now we thread the owner context into body resolution too.
        let mut owner_self_kind: HashMap<crate::hir::DefId, crate::hir::HirSelfKind> =
            HashMap::new();
        // Stage 18.54: Build a map from owner DefId → generic type params,
        // so body resolution can enter the owner's generic scope when
        // resolving param types and body type annotations.
        let mut owner_generic_params: HashMap<crate::hir::DefId, Vec<(Spur, usize)>> =
            HashMap::new();
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(item) = node {
                // Collect owner_self_kind for Trait/Impl owners only.
                if let Some((owner_def_id, kind)) = match item {
                    HirItem::Trait(t) => Some((t.hir_id.owner, crate::hir::HirSelfKind::Trait)),
                    HirItem::Impl(i) => Some((i.hir_id.owner, crate::hir::HirSelfKind::Impl)),
                    _ => None,
                } {
                    owner_self_kind.insert(owner_def_id, kind);
                }

                // Stage 18.56: Build trait_assoc_types map for qualified path
                // validation. For each trait, collect the names of its assoc types.
                // Per §1.0 原則 6 "通用 > 特例": one map for all traits.
                if let HirItem::Trait(t) = item {
                    let mut assoc_names = std::collections::HashSet::new();
                    for trait_item in &t.items {
                        if let crate::hir::HirTraitItem::Type(assoc) = trait_item {
                            assoc_names.insert(assoc.ident.name);
                        }
                    }
                    self.trait_assoc_types.insert(t.hir_id.owner, assoc_names);
                }

                // Stage 18.54: Collect generic type params for fn/struct/enum/trait/impl owners.
                // Per §1.0 原則 6 "通用 > 特例": one match handles all five owner kinds.
                let generic_params = match item {
                    HirItem::Fn(f) => Some(collect_generic_type_params(&f.generics)),
                    HirItem::Struct(s) => Some(collect_generic_type_params(&s.generics)),
                    HirItem::Enum(e) => Some(collect_generic_type_params(&e.generics)),
                    HirItem::Trait(t) => Some(collect_generic_type_params(&t.generics)),
                    HirItem::Impl(i) => Some(collect_generic_type_params(&i.generics)),
                    _ => None,
                };
                if let Some(params) = generic_params {
                    let owner_def_id = match item {
                        HirItem::Fn(f) => f.hir_id.owner,
                        HirItem::Struct(s) => s.hir_id.owner,
                        HirItem::Enum(e) => e.hir_id.owner,
                        HirItem::Trait(t) => t.hir_id.owner,
                        HirItem::Impl(i) => i.hir_id.owner,
                        _ => unreachable!(), // guarded by generic_params being Some
                    };
                    owner_generic_params.insert(owner_def_id, params);
                }
            }
        }

        // Walk all owners.
        for (_, node) in hir.owners.iter_mut() {
            self.resolve_owner_paths(node, interner);
        }
        // Walk all bodies — set owner context from the map.
        for (_, body) in hir.bodies.iter_mut() {
            self.current_self_kind = owner_self_kind.get(&body.hir_id.owner).copied();
            // Stage 18.54: Enter the owner's generic scope so type params
            // in param types and body annotations resolve correctly.
            if let Some(params) = owner_generic_params.get(&body.hir_id.owner) {
                self.generic_param_scope.push(params.clone());
            }
            self.resolve_body(body, interner);
            // Stage 18.54: Exit the owner's generic scope.
            if owner_generic_params.contains_key(&body.hir_id.owner) {
                self.generic_param_scope.pop();
            }
        }
        // Reset after all bodies.
        self.current_self_kind = None;
    }

    pub(super) fn resolve_owner_paths(&mut self, node: &mut OwnerNode, interner: &Rodeo) {
        if let OwnerNode::Item(item) = node {
            self.resolve_item_paths(item, interner);
        }
    }

    pub(super) fn resolve_item_paths(&mut self, item: &mut HirItem, interner: &Rodeo) {
        match item {
            HirItem::Fn(f) => {
                // Stage 14.40: extracted to `resolve_fn_sig_paths` for reuse
                // by `resolve_trait_item_paths` / `resolve_impl_item_paths`
                // (the inline clones inside Trait/Impl blocks).
                // Stage 18.54: enter generic scope so type params (T, U, ...)
                // in the signature resolve to Res::GenericParam.
                self.enter_generic_scope(&f.generics);
                self.resolve_fn_sig_paths(&mut f.sig, &mut f.generics, interner);
                self.exit_generic_scope();
            }
            HirItem::Const(c) => {
                self.resolve_ty_paths(&mut c.ty, interner);
            }
            HirItem::Static(s) => {
                self.resolve_ty_paths(&mut s.ty, interner);
            }
            HirItem::Struct(s) => {
                // Stage 18.54: enter generic scope so field types like `T`
                // in `struct S<T> { x: T }` resolve correctly.
                self.enter_generic_scope(&s.generics);
                self.resolve_generics_paths(&mut s.generics, interner);
                for field in &mut s.fields {
                    self.resolve_ty_paths(&mut field.ty, interner);
                }
                self.exit_generic_scope();
            }
            HirItem::Enum(e) => {
                // Stage 18.54: enter generic scope for enum variant field types.
                self.enter_generic_scope(&e.generics);
                self.resolve_generics_paths(&mut e.generics, interner);
                for variant in &mut e.variants {
                    match &mut variant.data {
                        HirVariantData::Tuple(fields, _) | HirVariantData::Struct(fields, _) => {
                            for field in fields {
                                self.resolve_ty_paths(&mut field.ty, interner);
                            }
                        }
                        _ => {}
                    }
                }
                self.exit_generic_scope();
            }
            HirItem::Trait(t) => {
                // Stage 3.66: set owner context so `Self` in supertrait bounds
                // resolves to `HirSelfKind::Trait`.
                // Stage 14.40: keep owner context set while processing trait
                // item signatures so `Self` in method signatures resolves to
                // `HirSelfKind::Trait` (previously only supertraits got the
                // context — items were left unresolved because the owner
                // copy was resolved but `trait.items` held an unresolved clone).
                // Stage 18.54: enter generic scope so trait's own type params
                // (e.g., `trait Foo<T>`) are visible in supertrait bounds and
                // item signatures.
                self.current_self_kind = Some(crate::hir::HirSelfKind::Trait);
                self.enter_generic_scope(&t.generics);
                self.resolve_generics_paths(&mut t.generics, interner);
                for bound in &mut t.supertraits {
                    if let HirTypeBound::Trait(tb) = bound {
                        self.resolve_hir_path(&mut tb.path, interner);
                    }
                }
                // Stage 14.40: resolve signatures of trait items (Fn/Const/Type)
                // inline. The owner copies (stored as separate HirItem::Fn
                // owners) get resolved by `resolve_item_paths(HirItem::Fn)`,
                // but `t.items` is a CLONE — it must be resolved independently.
                for trait_item in &mut t.items {
                    self.resolve_trait_item_paths(trait_item, interner);
                }
                self.exit_generic_scope();
                self.current_self_kind = None;
            }
            HirItem::Impl(i) => {
                // Stage 3.66: set owner context so `Self` in self_ty / of_trait
                // resolves to `HirSelfKind::Impl`.
                // Stage 14.40: keep owner context set while processing impl
                // item signatures so `Self` in method signatures resolves to
                // `HirSelfKind::Impl` (previously: only self_ty/of_trait got
                // the context; items were left unresolved because the owner
                // copy was resolved but `i.items` held an unresolved clone).
                // Stage 18.54: enter generic scope so impl's own type params
                // (e.g., `impl<T> Trait for T`) are visible in self_ty and
                // item signatures.
                self.current_self_kind = Some(crate::hir::HirSelfKind::Impl);
                self.enter_generic_scope(&i.generics);
                self.resolve_generics_paths(&mut i.generics, interner);
                self.resolve_ty_paths(&mut i.self_ty, interner);
                if let Some(trait_path) = &mut i.of_trait {
                    self.resolve_hir_path(trait_path, interner);
                }
                // Stage 14.40: resolve signatures of impl items (Fn/Const/Type)
                // inline. The owner copies (stored as separate HirItem::Fn
                // owners) get resolved by `resolve_item_paths(HirItem::Fn)`,
                // but `i.items` is a CLONE — it must be resolved independently.
                // This fixes the long-standing bug where impl method return
                // types like `fn add(...) -> V` had `path.res = Unknown`,
                // breaking method chain resolution in MIR lower.
                for impl_item in &mut i.items {
                    self.resolve_impl_item_paths(impl_item, interner);
                }
                self.exit_generic_scope();
                self.current_self_kind = None;
            }
            HirItem::TypeAlias(t) => {
                // Stage 18.54: enter generic scope for type alias's own params.
                self.enter_generic_scope(&t.generics);
                self.resolve_generics_paths(&mut t.generics, interner);
                self.resolve_ty_paths(&mut t.ty, interner);
                self.exit_generic_scope();
            }
            _ => {}
        }
    }

    /// Stage 14.40: Resolve signature paths inside a trait item.
    ///
    /// Trait items (Fn/Type/Const) are stored BOTH as separate owners AND as
    /// clones inside `HirTrait.items`. The owner copies get resolved by the
    /// `HirItem::Fn`/`HirItem::Const`/`HirItem::TypeAlias` arms of
    /// `resolve_item_paths`; this helper resolves the inline clones inside the
    /// trait block so that downstream passes reading `trait.items` see
    /// `Res::Def` instead of `Res::Unknown`.
    ///
    /// Per §13.4 + §14.4 (interface isolation): traits own their item
    /// signatures; the owner-copy duplication is an internal HIR lowering
    /// detail, not a resolver concern.
    pub(super) fn resolve_trait_item_paths(
        &mut self,
        item: &mut crate::hir::HirTraitItem,
        interner: &Rodeo,
    ) {
        match item {
            crate::hir::HirTraitItem::Fn(f) => {
                self.resolve_fn_sig_paths(&mut f.sig, &mut f.generics, interner);
            }
            crate::hir::HirTraitItem::Const(c) => {
                self.resolve_ty_paths(&mut c.ty, interner);
            }
            crate::hir::HirTraitItem::Type(t) => {
                // Stage 18.52 GATs Phase 1: resolve paths in generics (where clause,
                // type param bounds) so that GAT declarations like
                // `type Item<'a> where Self: 'a;` get their paths resolved.
                self.resolve_generics_paths(&mut t.generics, interner);
                for bound in &mut t.bounds {
                    if let HirTypeBound::Trait(tb) = bound {
                        self.resolve_hir_path(&mut tb.path, interner);
                    }
                }
                if let Some(default) = &mut t.default {
                    self.resolve_ty_paths(default, interner);
                }
            }
        }
    }

    /// Stage 14.40: Resolve signature paths inside an impl item.
    ///
    /// Same rationale as `resolve_trait_item_paths` — impl items are stored
    /// both as separate owners and as clones inside `HirImpl.items`. This
    /// helper resolves the inline clones so `query_method_return_type` and
    /// other MIR-lower queries reading `impl.items` see `Res::Def`.
    pub(super) fn resolve_impl_item_paths(
        &mut self,
        item: &mut crate::hir::HirImplItem,
        interner: &Rodeo,
    ) {
        match item {
            crate::hir::HirImplItem::Fn(f) => {
                self.resolve_fn_sig_paths(&mut f.sig, &mut f.generics, interner);
            }
            crate::hir::HirImplItem::Const(c) => {
                self.resolve_ty_paths(&mut c.ty, interner);
            }
            crate::hir::HirImplItem::Type(t) => {
                // Stage 18.52 GATs Phase 1: resolve generics paths in impl-side
                // GAT bindings (`impl Trait { type Item<'a> = &'a T; }`).
                self.resolve_generics_paths(&mut t.generics, interner);
                for bound in &mut t.bounds {
                    if let HirTypeBound::Trait(tb) = bound {
                        self.resolve_hir_path(&mut tb.path, interner);
                    }
                }
                if let Some(default) = &mut t.default {
                    self.resolve_ty_paths(default, interner);
                }
            }
        }
    }

    /// Stage 14.40: Resolve all paths in a function signature
    /// (generics + input param types + return type).
    ///
    /// Extracted from `resolve_item_paths(HirItem::Fn)` so it can be reused
    /// by `resolve_trait_item_paths` and `resolve_impl_item_paths` for the
    /// inline clones inside Trait/Impl blocks.
    pub(super) fn resolve_fn_sig_paths(
        &mut self,
        sig: &mut crate::hir::HirFnSig,
        generics: &mut HirGenerics,
        interner: &Rodeo,
    ) {
        self.resolve_generics_paths(generics, interner);
        for param in &mut sig.inputs {
            if let Some(ty) = &mut param.ty {
                self.resolve_ty_paths(ty, interner);
            }
        }
        if let crate::hir::HirFnRetTy::Ty(ty) = &mut sig.output {
            self.resolve_ty_paths(ty, interner);
        }
    }

    pub(super) fn resolve_generics_paths(&mut self, generics: &mut HirGenerics, interner: &Rodeo) {
        for param in &mut generics.params {
            if let HirGenericParam::Type(tp) = param {
                for bound in &mut tp.bounds {
                    if let HirTypeBound::Trait(tb) = bound {
                        self.resolve_hir_path(&mut tb.path, interner);
                    }
                }
                if let Some(default) = &mut tp.default {
                    self.resolve_ty_paths(default, interner);
                }
            }
        }
        for pred in &mut generics.where_clause {
            self.resolve_ty_paths(&mut pred.bounded_ty, interner);
            for bound in &mut pred.bounds {
                if let HirTypeBound::Trait(tb) = bound {
                    self.resolve_hir_path(&mut tb.path, interner);
                }
            }
        }
    }

    pub(super) fn resolve_ty_paths(&mut self, ty: &mut HirTy, interner: &Rodeo) {
        match &mut ty.kind {
            HirTyKind::Tuple(tys) => {
                for t in tys {
                    self.resolve_ty_paths(t, interner);
                }
            }
            HirTyKind::Array(t, _) | HirTyKind::Slice(t) => {
                self.resolve_ty_paths(t, interner);
            }
            HirTyKind::Ref(_, _, t) | HirTyKind::Ptr(_, t) => {
                self.resolve_ty_paths(t, interner);
            }
            HirTyKind::FnPtr { inputs, output, .. } => {
                for t in inputs {
                    self.resolve_ty_paths(t, interner);
                }
                self.resolve_ty_paths(output, interner);
            }
            HirTyKind::Path(qself, path) => {
                // Stage 18.56: Handle qualified paths `<T as Trait>::Item`.
                // Per §1.0 原則 9 "正确 > 妥协": trait qualifier must be
                // respected — previously `qself` was ignored via `_`.
                // Per §1.0 原則 4 "报错 > 静默": if the assoc type is not
                // found in the trait, emit a resolve error.
                if let Some(inner_ty) = &mut qself.ty {
                    // Qualified path: resolve inner type first.
                    self.resolve_ty_paths(inner_ty, interner);
                    // Resolve the trait name (first segment after qself.position
                    // boundary — for `<T as Trait>::Item`, position=1 means
                    // segments[0] is the trait).
                    let trait_res = if qself.position > 0 && qself.position <= path.segments.len() {
                        let trait_seg = &path.segments[0];
                        // Look up the trait name in the type namespace.
                        if let Some(def_id) = self.module_tree.lookup_type(trait_seg.ident.name) {
                            let kind = self
                                .def_kinds
                                .get(&def_id)
                                .copied()
                                .unwrap_or(DefKind::Trait);
                            Res::Def(def_id, kind)
                        } else {
                            Res::Err
                        }
                    } else {
                        Res::Err
                    };
                    // Check if the assoc type exists in the resolved trait.
                    let assoc_name = path.segments.last().map(|s| s.ident.name);
                    if let (Res::Def(trait_def_id, _), Some(assoc_name)) = (trait_res, assoc_name) {
                        if self.assoc_type_exists_in_trait(trait_def_id, assoc_name) {
                            // Assoc type found — mark path as resolved.
                            path.res = Res::Def(trait_def_id, crate::hir::DefKind::Trait);
                        } else {
                            // Assoc type not found in trait — report error.
                            // Per §1.0 原則 4 "报错 > 静默".
                            path.res = Res::Err;
                            self.errors.push(crate::resolve::ResolveError::with_kind(
                                crate::resolve::ResolveErrorKind::AssocTypeNotFound,
                                format!(
                                    "associated type `{}` not found in trait",
                                    interner.resolve(&assoc_name)
                                ),
                                path.span,
                            ));
                        }
                    } else if !matches!(trait_res, Res::Def(_, _)) && qself.position > 0 {
                        // Trait itself not found — report error.
                        path.res = Res::Err;
                        self.errors.push(crate::resolve::ResolveError::with_kind(
                            crate::resolve::ResolveErrorKind::UndefinedTraitInQualified,
                            "cannot find trait in qualified path".to_string(),
                            path.span,
                        ));
                    }
                    // Stage 18.56: Recursively resolve generic args on path segments.
                    // Per §1.0 原則 2 "整体 > 局部": covers nested types in
                    // qualified path segments (e.g., `<T as C>::Item<NestedType>`).
                    for seg in &mut path.segments {
                        self.resolve_segment_args(seg, interner);
                    }
                } else {
                    // Plain path: existing behavior.
                    self.resolve_hir_path(path, interner);
                    // Stage 18.56: Recursively resolve generic args on path segments.
                    // Per §1.0 原則 2 "整体 > 局部": covers nested types like
                    // `Vec<<T as C>::Item>` where the inner qualified path is
                    // a generic arg of the outer path.
                    for seg in &mut path.segments {
                        self.resolve_segment_args(seg, interner);
                    }
                }
            }
            HirTyKind::TraitObject { bounds, .. } | HirTyKind::ImplTrait(bounds) => {
                for bound in bounds {
                    if let HirTypeBound::Trait(tb) = bound {
                        self.resolve_hir_path(&mut tb.path, interner);
                    }
                }
            }
            _ => {}
        }
    }

    /// Stage 18.56: Recursively resolve generic args on a path segment.
    ///
    /// Handles nested types in generic args like `Vec<<T as C>::Item>` where
    /// the inner qualified path is a type arg of the outer path. Previously
    /// these nested types were not resolved (existing gap — pre-Stage 18.56).
    ///
    /// Per §1.0 原則 2 "整体 > 局部": resolves the full type tree, not just
    /// the top-level path.
    /// Per §10 naming: `resolve_segment_args` follows `<verb>_<noun>_<noun>` pattern.
    pub(super) fn resolve_segment_args(
        &mut self,
        seg: &mut crate::hir::HirPathSegment,
        interner: &Rodeo,
    ) {
        if let Some(crate::ast::GenericArgs::AngleBracketed(args)) = &seg.args {
            for arg in args.iter() {
                if let crate::ast::GenericArg::Type(ty) = arg {
                    // The arg is an AST Ty — we need to resolve it as a HIR Ty.
                    // Since the arg is AST (not HIR), we lower it temporarily
                    // and resolve. For Stage 18.56, we use a simplified approach:
                    // walk the AST Ty and resolve any path segments.
                    self.resolve_ast_ty_paths(ty, interner);
                }
            }
        }
    }

    /// Stage 18.56: Recursively resolve paths in an AST Ty (used for generic
    /// args which are stored as AST, not HIR).
    ///
    /// Note: The `interner` parameter is passed through for future use (e.g.,
    /// resolving AST path names to DefIds). Currently it's only used in
    /// recursive calls to maintain the signature.
    ///
    /// Per §1.0 原則 6 "通用 > 特例": handles all AST Ty variants.
    #[allow(clippy::only_used_in_recursion)]
    fn resolve_ast_ty_paths(&mut self, ty: &crate::ast::Ty, interner: &Rodeo) {
        use crate::ast::Ty as ATy;
        match ty {
            ATy::Path(qself, path, _) => {
                // For AST paths, we can't set res (AST has no res field).
                // But we can resolve the generic args on the path segments.
                if let Some(inner_ty) = &qself.ty {
                    self.resolve_ast_ty_paths(inner_ty, interner);
                }
                for seg in &path.segments {
                    if let Some(crate::ast::GenericArgs::AngleBracketed(arg_list)) = &seg.args {
                        for arg in arg_list.iter() {
                            if let crate::ast::GenericArg::Type(t) = arg {
                                self.resolve_ast_ty_paths(t, interner);
                            }
                        }
                    }
                }
            }
            ATy::Ref(_, _, inner, _) | ATy::Ptr(_, inner, _) | ATy::Slice(inner, _) => {
                self.resolve_ast_ty_paths(inner, interner);
            }
            ATy::Array(inner, _, _) => {
                self.resolve_ast_ty_paths(inner, interner);
            }
            ATy::Tuple(tys, _) => {
                for t in tys {
                    self.resolve_ast_ty_paths(t, interner);
                }
            }
            _ => {}
        }
    }

    pub(super) fn resolve_hir_path(&mut self, path: &mut HirPath, interner: &Rodeo) {
        // Stage 18.54: Re-resolve Err paths too. Previously, once a path was
        // marked Err (e.g., during body param resolution before generic scope
        // was entered), it would never be re-tried. Now we re-resolve Err
        // paths so they can benefit from generic scope information that may
        // have been added since the first failed attempt.
        // Per §1.0 原則 4 "报错 > 静默": but also per §1.0 原則 9 "正确 > 妥协" —
        // we don't silently accept; we re-try with better info.
        if !matches!(path.res, Res::Unknown | Res::Err) {
            return;
        }
        path.res = self.resolve_path(path, interner);
    }

    /// Core path resolution: look up a HirPath in the module tree + scope chain.
    pub(super) fn resolve_path(&self, path: &HirPath, interner: &Rodeo) -> Res {
        if path.segments.is_empty() {
            return Res::Err;
        }

        // Stage 18.21: Recognize `__landin_<name>` as known runtime functions.
        // These are emitted by the built-in macro_rules! definitions
        // (e.g., `println!(...)` expands to `__landin_println(...)`).
        // Without this, the resolver would report "cannot find value" for
        // every println!/print!/eprintln!/eprint! call.
        // Per §1.0 原則 6 "通用 > 特解": one check for all __landin_ functions.
        if path.segments.len() == 1 && path.leading == PathLeading::None {
            let name = interner.resolve(&path.segments[0].ident.name);
            if name.starts_with("__landin_") {
                // Return a synthetic Def for the runtime function.
                // The driver registers each __landin_<name> in
                // fn_name_by_def_id with a synthetic DefId(u32::MAX - i).
                // We use the same scheme here so codegen can resolve the name.
                // For __landin_println → DefId(u32::MAX - 0)
                // For __landin_print   → DefId(u32::MAX - 1)
                // For __landin_eprintln→ DefId(u32::MAX - 2)
                // For __landin_eprint  → DefId(u32::MAX - 3)
                // For other __landin_  → DefId(u32::MAX) (fallback)
                let builtin_names = [
                    "__landin_println",
                    "__landin_print",
                    "__landin_eprintln",
                    "__landin_eprint",
                ];
                let idx = builtin_names.iter().position(|n| *n == name);
                let def_id = match idx {
                    Some(i) => crate::hir::DefId::new(u32::MAX - i as u32),
                    None => crate::hir::DefId::new(u32::MAX),
                };
                return Res::Def(def_id, crate::hir::DefKind::Fn);
            }
        }

        // Single-segment, no leading prefix: could be primitive, local name, or Self.
        if path.segments.len() == 1 && path.leading == PathLeading::None {
            let seg = &path.segments[0];
            let name = interner.resolve(&seg.ident.name);

            // Stage 1.4: Check local scope FIRST (before module-level items).
            // Locals shadow items (e.g., `let i32 = 42;` shadows the `i32` type —
            // though that's unusual, the resolution order is: local → primitive → item).
            if let Some(scopes) = &self.scopes {
                if let Some(hir_id) = scopes.lookup(seg.ident.name) {
                    return Res::Local(hir_id);
                }
            }

            // Stage 18.54: Check generic type parameter scope before primitives.
            // A user-named `T` in `fn f<T>(x: T)` should resolve to the generic
            // param, not fall through to Res::Err.
            // Per §1.0 原則 6 "通用 > 特例": one lookup for all owner kinds.
            // Per §1.0 原則 3 "显式 > 隐式": explicit Res::GenericParam variant.
            if let Some(idx) = self.lookup_generic_param(seg.ident.name) {
                return Res::GenericParam(seg.ident.name, idx);
            }

            // Primitive types.
            if let Some(prim) = lookup_prim_ty(name) {
                return Res::PrimTy(prim);
            }

            // Self type keyword.
            // Stage 3.65: now carries HirSelfKind to distinguish trait-Self
            // from impl-Self.
            // Stage 3.66: uses `current_self_kind` context (set by
            // `resolve_item_paths` when entering Trait/Impl items) to
            // produce the accurate variant. Defaults to `Impl` when no
            // owner context is active (e.g., body-level resolution —
            // threading owner context into body resolution is Stage 4).
            if let Some(self_spur) = interner.get("Self") {
                if seg.ident.name == self_spur {
                    return Res::SelfTy(
                        self.current_self_kind
                            .unwrap_or(crate::hir::HirSelfKind::Impl),
                    );
                }
            }
            if name == "Self" {
                return Res::SelfTy(
                    self.current_self_kind
                        .unwrap_or(crate::hir::HirSelfKind::Impl),
                );
            }

            // Value namespace (fn, const, static).
            if let Some(def_id) = self.module_tree.lookup_value(seg.ident.name) {
                // Stage 3.30: look up DefKind from the def_kinds table so
                // downstream passes (MIR lower, codegen) can distinguish
                // fn calls from struct ctors without re-querying HIR.
                let kind = self.def_kinds.get(&def_id).copied().unwrap_or(DefKind::Fn);
                // Stage 3.68: visibility check (stub — currently always Ok).
                let _ = self.check_visibility(def_id, path.span);
                return Res::Def(def_id, kind);
            }

            // Type namespace (struct, enum, trait, type alias, mod).
            if let Some(def_id) = self.module_tree.lookup_type(seg.ident.name) {
                let kind = self
                    .def_kinds
                    .get(&def_id)
                    .copied()
                    .unwrap_or(DefKind::Struct);
                // Stage 3.68: visibility check (stub — currently always Ok).
                let _ = self.check_visibility(def_id, path.span);
                return Res::Def(def_id, kind);
            }

            // Stage 3.64: Use imports (`use a::b::c;` or `use a::b::*;`).
            // Consult the use_imports table as a fallback. Leaf imports
            // shadow glob imports (handled by `insert_use_import`).
            if let Some(import) = self.module_tree.lookup_use_import(seg.ident.name) {
                return Res::Def(import.target, import.kind);
            }

            // Not found.
            return Res::Err;
        }

        // Multi-segment path: resolve first segment, then walk.
        let first = &path.segments[0];
        let first_def = self
            .module_tree
            .lookup_type(first.ident.name)
            .or_else(|| self.module_tree.lookup_value(first.ident.name))
            // Stage 3.64: also check use_imports for the first segment.
            .or_else(|| {
                self.module_tree
                    .lookup_use_import(first.ident.name)
                    .map(|imp| imp.target)
            });

        if let Some(def_id) = first_def {
            // Stage 14.41: For 2-segment paths like `V::new`, check the
            // impl_method_index BEFORE falling back to returning the type's
            // DefId. This fixes the long-standing bug where `V::new(1, 2)`
            // was treated as a struct constructor `V { x: 1, y: 2 }` instead
            // of calling the `new` method.
            //
            // The index is built during `build_module_tree` (Phase 1) and
            // keyed by `(type_name, method_name)`. We only check this for
            // 2-segment paths where the first segment resolves to a struct/
            // enum (DefKind::Struct or DefKind::Enum). For other DefKinds
            // (Mod, Fn, etc.), the original behavior is preserved.
            //
            // Per §16 (interface isolation): the resolver doesn't need HIR
            // access — the index is pre-computed data. Per §13.4 (design
            // alignment): this is the proper way to resolve `Type::method`
            // paths, not the previous "return first segment's DefId" hack.
            if path.segments.len() == 2 {
                let first_kind = self.def_kinds.get(&def_id).copied();
                if matches!(first_kind, Some(DefKind::Struct) | Some(DefKind::Enum)) {
                    let type_name = first.ident.name;
                    let method_name = path.segments[1].ident.name;
                    if let Some(&method_def_id) =
                        self.impl_method_index.get(&(type_name, method_name))
                    {
                        // Found the method! Return it as Res::Def with DefKind::Fn.
                        let _ = self.check_visibility(method_def_id, path.span);
                        return Res::Def(method_def_id, DefKind::Fn);
                    }
                    // Method not found in impl_method_index — fall through to
                    // the original behavior (return the type's DefId). This
                    // handles cases like `Color::Red` (enum variant access)
                    // where the second segment is a variant, not a method.
                }
            }

            // For multi-segment paths where the first segment is a module,
            // we would walk into the child module. For Stage 1.3, we resolve
            // the first segment and return — full multi-level resolution
            // (e.g., `std::io::Read`) requires cross-crate resolution which
            // is Stage 5+ work.
            // Stage 3.30: include DefKind (per §15).
            let kind = self.def_kinds.get(&def_id).copied().unwrap_or(DefKind::Mod);
            return Res::Def(def_id, kind);
        }

        // Check if first segment is a primitive type (e.g., `i32::MAX`).
        let first_name = interner.resolve(&first.ident.name);
        if let Some(prim) = lookup_prim_ty(first_name) {
            return Res::PrimTy(prim);
        }

        Res::Err
    }

    // ================================================================
    // Body + expression resolution (Stage 1.4: with scope tracking)
    // ================================================================

    pub(super) fn resolve_body(&mut self, body: &mut Body, interner: &Rodeo) {
        // Create a Fn scope for the body.
        self.scopes = Some(ScopeStack::new(ScopeKind::Fn));

        // Register fn params as bindings in the Fn scope.
        for param in &mut body.params {
            self.collect_pat_bindings(&mut param.pat, interner);
            if let Some(ty) = &mut param.ty {
                self.resolve_ty_paths(ty, interner);
            }
        }

        // Resolve the body expression with scope tracking.
        self.resolve_expr(&mut body.value, interner);

        // Pop the Fn scope.
        self.scopes = None;
    }

    /// Collect all identifier bindings from a pattern into the current scope.
    /// Stage 3.40 (L-ENUM-MATCH): also resolve pattern paths (e.g.,
    /// `Color::Red` in `match c { Color::Red => ... }`).
    pub(super) fn collect_pat_bindings(&mut self, pat: &mut HirPat, interner: &Rodeo) {
        match &mut pat.kind {
            HirPatKind::Ident(_mode, ident, sub) => {
                if let Some(scopes) = &mut self.scopes {
                    scopes.insert(ident.name, pat.hir_id);
                }
                if let Some(sub) = sub {
                    self.collect_pat_bindings(sub, interner);
                }
            }
            HirPatKind::Struct(path, fields, _rest) => {
                self.resolve_hir_path(path, interner);
                for f in fields {
                    self.collect_pat_bindings(&mut f.pat, interner);
                }
            }
            HirPatKind::TupleStruct(path, pats) => {
                self.resolve_hir_path(path, interner);
                for p in pats {
                    self.collect_pat_bindings(p, interner);
                }
            }
            HirPatKind::Tuple(pats) => {
                for p in pats {
                    self.collect_pat_bindings(p, interner);
                }
            }
            HirPatKind::Slice(pats, rest) => {
                for p in pats {
                    self.collect_pat_bindings(p, interner);
                }
                if let Some(r) = rest {
                    self.collect_pat_bindings(r, interner);
                }
            }
            HirPatKind::Or(pats) => {
                if let Some(first) = pats.first_mut() {
                    self.collect_pat_bindings(first, interner);
                }
            }
            HirPatKind::Ref(pat, _) => {
                self.collect_pat_bindings(pat, interner);
            }
            HirPatKind::Path(path) => {
                self.resolve_hir_path(path, interner);
            }
            HirPatKind::Lit(_) | HirPatKind::Wild | HirPatKind::Rest => {}
            HirPatKind::Range(_, _, _) => {}
        }
    }

    pub(super) fn resolve_expr(&mut self, expr: &mut HirExpr, interner: &Rodeo) {
        match &mut expr.kind {
            HirExprKind::Lit(_) | HirExprKind::Unit | HirExprKind::Continue => {}
            HirExprKind::Path(p) => {
                self.resolve_hir_path(p, interner);
            }
            HirExprKind::Block(b) => self.resolve_block(b, interner),
            HirExprKind::Call { func, args } => {
                self.resolve_expr(func, interner);
                for a in args {
                    self.resolve_expr(a, interner);
                }
            }
            HirExprKind::MethodCall { receiver, args, .. } => {
                self.resolve_expr(receiver, interner);
                for a in args {
                    self.resolve_expr(a, interner);
                }
            }
            HirExprKind::Field { receiver, .. } => self.resolve_expr(receiver, interner),
            HirExprKind::Index { receiver, index } => {
                self.resolve_expr(receiver, interner);
                self.resolve_expr(index, interner);
            }
            HirExprKind::Unary { expr, .. } => self.resolve_expr(expr, interner),
            HirExprKind::Binary { lhs, rhs, .. } => {
                self.resolve_expr(lhs, interner);
                self.resolve_expr(rhs, interner);
            }
            HirExprKind::Assign { lhs, rhs, .. } => {
                self.resolve_expr(lhs, interner);
                self.resolve_expr(rhs, interner);
            }
            HirExprKind::AddrOf { expr, .. } => self.resolve_expr(expr, interner),
            HirExprKind::Cast { expr, ty } => {
                self.resolve_expr(expr, interner);
                self.resolve_ty_paths(ty, interner);
            }
            HirExprKind::Try { expr } => self.resolve_expr(expr, interner),
            HirExprKind::If { cond, then, else_ } => {
                self.resolve_expr(cond, interner);
                self.resolve_block(then, interner);
                if let Some(e) = else_ {
                    self.resolve_expr(e, interner);
                }
            }
            HirExprKind::Match { expr, arms } => {
                self.resolve_expr(expr, interner);
                for arm in arms {
                    // Push a MatchArm scope for pattern bindings.
                    if let Some(scopes) = &mut self.scopes {
                        scopes.push(ScopeKind::MatchArm);
                    }
                    self.collect_pat_bindings(&mut arm.pat, interner);
                    if let Some(g) = &mut arm.guard {
                        self.resolve_expr(g, interner);
                    }
                    self.resolve_expr(&mut arm.body, interner);
                    // Pop the MatchArm scope.
                    if let Some(scopes) = &mut self.scopes {
                        scopes.pop();
                    }
                }
            }
            HirExprKind::Loop { body } => {
                if let Some(scopes) = &mut self.scopes {
                    scopes.push(ScopeKind::Loop);
                }
                self.resolve_block(body, interner);
                if let Some(scopes) = &mut self.scopes {
                    scopes.pop();
                }
            }
            HirExprKind::While { cond, body } => {
                self.resolve_expr(cond, interner);
                if let Some(scopes) = &mut self.scopes {
                    scopes.push(ScopeKind::Loop);
                }
                self.resolve_block(body, interner);
                if let Some(scopes) = &mut self.scopes {
                    scopes.pop();
                }
            }
            HirExprKind::For { pat, iter, body } => {
                self.resolve_expr(iter, interner);
                if let Some(scopes) = &mut self.scopes {
                    scopes.push(ScopeKind::Loop);
                }
                self.collect_pat_bindings(pat, interner);
                self.resolve_block(body, interner);
                if let Some(scopes) = &mut self.scopes {
                    scopes.pop();
                }
            }
            HirExprKind::Closure { params, body, .. } => {
                // Push a Closure scope for closure params.
                if let Some(scopes) = &mut self.scopes {
                    scopes.push(ScopeKind::Closure);
                }
                for param in params {
                    self.collect_pat_bindings(&mut param.pat, interner);
                }
                self.resolve_expr(body, interner);
                if let Some(scopes) = &mut self.scopes {
                    scopes.pop();
                }
            }
            HirExprKind::Return { expr } | HirExprKind::Break { expr } => {
                if let Some(e) = expr {
                    self.resolve_expr(e, interner);
                }
            }
            HirExprKind::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.resolve_expr(s, interner);
                }
                if let Some(e) = end {
                    self.resolve_expr(e, interner);
                }
            }
            HirExprKind::Tuple { elems } | HirExprKind::Array { elems } => {
                for e in elems {
                    self.resolve_expr(e, interner);
                }
            }
            HirExprKind::Repeat { elem, count } => {
                self.resolve_expr(elem, interner);
                self.resolve_expr(count, interner);
            }
            HirExprKind::Struct { path, fields } => {
                self.resolve_hir_path(path, interner);
                for f in fields {
                    if let Some(e) = &mut f.expr {
                        self.resolve_expr(e, interner);
                    }
                }
            }
            HirExprKind::MacroCall { path, .. } => {
                self.resolve_hir_path(path, interner);
            }
            // Stage 13.16: resolve paths inside Println args (format args).
            // Before Stage 13.16, Println carried only `msg: String` (no args),
            // so there was nothing to resolve. Now that Println carries
            // Stage 18.48: HirExprKind::Println variant removed.
            HirExprKind::Unsafe(b) => self.resolve_block(b, interner),
            // Stage 8.5: async/await — resolve inner expressions
            HirExprKind::Await { expr } => self.resolve_expr(expr, interner),
            HirExprKind::Async { block } => self.resolve_block(block, interner),
        }
    }

    pub(super) fn resolve_block(&mut self, block: &mut HirBlock, interner: &Rodeo) {
        // Push a Block scope for let bindings.
        if let Some(scopes) = &mut self.scopes {
            scopes.push(ScopeKind::Block);
        }

        for stmt in &mut block.stmts {
            match stmt {
                HirStmt::Local(local) => {
                    // Resolve the type annotation (if any) BEFORE registering
                    // the binding — the type is looked up in the current scope.
                    if let Some(ty) = &mut local.ty {
                        self.resolve_ty_paths(ty, interner);
                    }
                    // Resolve the init expression BEFORE registering the binding.
                    // This prevents forward references: `let x = x;` should resolve
                    // the `x` on the right to an OUTER binding (or Err if none),
                    // NOT to the binding being created.
                    if let Some(init) = &mut local.init {
                        self.resolve_expr(init, interner);
                    }
                    // NOW register the binding in the current scope.
                    // After this point, references to the name resolve to this binding.
                    self.collect_pat_bindings(&mut local.pat, interner);
                }
                HirStmt::Expr(e, _) => self.resolve_expr(e, interner),
                _ => {}
            }
        }
        if let Some(expr) = &mut block.expr {
            self.resolve_expr(expr, interner);
        }

        // Pop the Block scope.
        if let Some(scopes) = &mut self.scopes {
            scopes.pop();
        }
    }
}

/// Stage 18.54: Collect generic type parameters from a `HirGenerics` as
/// `(name, index)` pairs.
///
/// Used by `resolve_all_paths` to build the `owner_generic_params` map so
/// body resolution can enter the owner's generic scope.
///
/// Per §10 naming: `collect_generic_type_params` follows
/// `<verb>_<adj>_<noun>_<noun>` pattern.
/// Per §1.0 原則 6 "通用 > 特例": one function for all owner kinds.
fn collect_generic_type_params(generics: &crate::hir::HirGenerics) -> Vec<(Spur, usize)> {
    let mut params = Vec::new();
    for (idx, param) in generics.params.iter().enumerate() {
        if let crate::hir::HirGenericParam::Type(tp) = param {
            params.push((tp.ident.name, idx));
        }
    }
    params
}
