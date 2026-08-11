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
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(item) = node {
                let (owner_def_id, kind) = match item {
                    HirItem::Trait(t) => (t.hir_id.owner, crate::hir::HirSelfKind::Trait),
                    HirItem::Impl(i) => (i.hir_id.owner, crate::hir::HirSelfKind::Impl),
                    _ => continue,
                };
                owner_self_kind.insert(owner_def_id, kind);
            }
        }

        // Walk all owners.
        for (_, node) in hir.owners.iter_mut() {
            self.resolve_owner_paths(node, interner);
        }
        // Walk all bodies — set owner context from the map.
        for (_, body) in hir.bodies.iter_mut() {
            self.current_self_kind = owner_self_kind.get(&body.hir_id.owner).copied();
            self.resolve_body(body, interner);
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
                self.resolve_fn_sig_paths(&mut f.sig, &mut f.generics, interner);
            }
            HirItem::Const(c) => {
                self.resolve_ty_paths(&mut c.ty, interner);
            }
            HirItem::Static(s) => {
                self.resolve_ty_paths(&mut s.ty, interner);
            }
            HirItem::Struct(s) => {
                self.resolve_generics_paths(&mut s.generics, interner);
                for field in &mut s.fields {
                    self.resolve_ty_paths(&mut field.ty, interner);
                }
            }
            HirItem::Enum(e) => {
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
            }
            HirItem::Trait(t) => {
                // Stage 3.66: set owner context so `Self` in supertrait bounds
                // resolves to `HirSelfKind::Trait`.
                // Stage 14.40: keep owner context set while processing trait
                // item signatures so `Self` in method signatures resolves to
                // `HirSelfKind::Trait` (previously only supertraits got the
                // context — items were left unresolved because the owner
                // copy was resolved but `trait.items` held an unresolved clone).
                self.current_self_kind = Some(crate::hir::HirSelfKind::Trait);
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
                self.current_self_kind = Some(crate::hir::HirSelfKind::Impl);
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
                self.current_self_kind = None;
            }
            HirItem::TypeAlias(t) => {
                self.resolve_generics_paths(&mut t.generics, interner);
                self.resolve_ty_paths(&mut t.ty, interner);
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
            HirTyKind::Path(_, path) => {
                self.resolve_hir_path(path, interner);
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

    pub(super) fn resolve_hir_path(&mut self, path: &mut HirPath, interner: &Rodeo) {
        if path.res != Res::Unknown {
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
            // `args: Vec<HirExpr>`, we must resolve each arg expression so
            // that path args (e.g., `println!("{}", x)`) get `Res::Local`
            // instead of `Res::Unknown` — otherwise MIR lower falls back to
            // an error placeholder (Const{val: Int(0), ty: Error}).
            HirExprKind::Println { args, .. } => {
                for arg in args {
                    self.resolve_expr(arg, interner);
                }
            }
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
