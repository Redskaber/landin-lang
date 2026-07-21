//! Name resolver: walks HIR and fills `Res` on all `HirPath` nodes.
//!
//! Public entry point: [`resolve_crate`].

use crate::ast::PathLeading;
use crate::hir::*;
use crate::resolve::error::ResolveError;
use crate::resolve::module_tree::{DefKind, ModuleNode, UseDecl, UseImport};
use crate::resolve::scope::{ScopeKind, ScopeStack};
use crate::session::Span;
use lasso::{Rodeo, Spur};
use std::collections::HashMap;

/// The name resolver. Holds the module tree, DefKind map, scope stack,
/// and errors.
#[derive(Default)]
pub struct Resolver {
    /// Module tree: crate root → nested mods.
    module_tree: ModuleNode,
    /// Map from DefId → DefKind (for namespace disambiguation).
    def_kinds: HashMap<DefId, DefKind>,
    /// Scope stack for local variable resolution (Stage 1.4).
    /// `None` when not inside a body (e.g., during module tree construction).
    scopes: Option<ScopeStack>,
    /// Errors encountered (non-fatal).
    errors: Vec<ResolveError>,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            module_tree: ModuleNode::new(),
            def_kinds: HashMap::new(),
            scopes: None,
            errors: Vec::new(),
        }
    }

    /// Resolve all paths in the HIR crate, mutating `HirPath.res` in-place.
    pub fn resolve(&mut self, hir: &mut HirCrate, interner: &Rodeo) {
        self.build_module_tree(hir, interner);
        self.resolve_uses();
        self.resolve_all_paths(hir, interner);
    }

    // ================================================================
    // Phase 1: Build module tree
    // ================================================================

    fn build_module_tree(&mut self, hir: &HirCrate, interner: &Rodeo) {
        // Collect registrations from all owners, then insert into module tree.
        // This avoids borrow conflicts (collecting first, then mutating).
        let mut registrations: Vec<(DefId, DefKind, Spur)> = Vec::new();
        let mut use_decls: Vec<UseDecl> = Vec::new();

        for (def_id, node) in &hir.owners {
            if let OwnerNode::Item(item) = node {
                match item {
                    HirItem::Fn(f) => {
                        registrations.push((*def_id, DefKind::Fn, f.ident.name));
                        self.def_kinds.insert(*def_id, DefKind::Fn);
                    }
                    HirItem::Const(c) => {
                        registrations.push((*def_id, DefKind::Const, c.ident.name));
                        self.def_kinds.insert(*def_id, DefKind::Const);
                    }
                    HirItem::Static(s) => {
                        registrations.push((*def_id, DefKind::Static, s.ident.name));
                        self.def_kinds.insert(*def_id, DefKind::Static);
                    }
                    HirItem::Struct(s) => {
                        registrations.push((*def_id, DefKind::Struct, s.ident.name));
                        self.def_kinds.insert(*def_id, DefKind::Struct);
                    }
                    HirItem::Enum(e) => {
                        registrations.push((*def_id, DefKind::Enum, e.ident.name));
                        self.def_kinds.insert(*def_id, DefKind::Enum);
                    }
                    HirItem::Trait(t) => {
                        registrations.push((*def_id, DefKind::Trait, t.ident.name));
                        self.def_kinds.insert(*def_id, DefKind::Trait);
                    }
                    HirItem::Impl(_) => {
                        self.def_kinds.insert(*def_id, DefKind::Impl);
                    }
                    HirItem::TypeAlias(t) => {
                        registrations.push((*def_id, DefKind::TypeAlias, t.ident.name));
                        self.def_kinds.insert(*def_id, DefKind::TypeAlias);
                    }
                    HirItem::ExternBlock(_) => {
                        self.def_kinds.insert(*def_id, DefKind::ExternFn);
                    }
                    HirItem::Mod(m) => {
                        registrations.push((*def_id, DefKind::Mod, m.ident.name));
                        self.def_kinds.insert(*def_id, DefKind::Mod);
                    }
                    HirItem::Use(u) => {
                        use_decls.push(UseDecl {
                            tree: u.tree.clone(),
                            vis: u.vis.clone(),
                            span: u.span,
                        });
                        self.def_kinds.insert(*def_id, DefKind::Use);
                    }
                }
            }
        }

        // Insert registrations into the module tree.
        // Note: we don't filter by Spur::default() because the first
        // interned symbol gets Spur(0) which equals Spur::default().
        // Items without names (impl/extern) are never added to
        // `registrations`, so all entries here have real names.
        for (def_id, kind, name) in registrations {
            if let Err(existing) = self.module_tree.insert(name, def_id, kind) {
                let name_str = interner.resolve(&name).to_string();
                self.errors.push(ResolveError::new(
                    format!(
                        "duplicate definition for `{}` (also defined at {:?})",
                        name_str, existing
                    ),
                    Span::DUMMY,
                ));
            }
        }

        // Store use declarations for later processing.
        self.module_tree.use_decls.extend(use_decls);
    }

    // ================================================================
    // Phase 2: Resolve use declarations (Stage 3.64)
    // ================================================================
    //
    // Previously (Stage 1.3-3.62): `resolve_uses` was a no-op stub that
    // just set `uses_resolved = true`. This meant `use a::b::c;` declarations
    // had no effect on path resolution — real Landin programs that used
    // imports couldn't compile.
    //
    // Stage 3.64: implements basic use resolution:
    // - Leaf imports (`use a::b::c;` or `use a::b::c as d;`) → register
    //   the imported name in `module_tree.use_imports`.
    // - Glob imports (`use a::b::*;`) → register all public items from
    //   the target module as glob imports (lower priority than leaf).
    // - Path-prefix imports (`use a::{b, c};`) → recurse into children.
    //
    // Limitations (deferred to Stage 4+):
    // - Cross-crate imports (Stage 5+).
    // - Visibility enforcement (Stage 1.3 Phase E1, still not implemented).
    // - Ambiguity detection at use-site (currently at import-site only).

    fn resolve_uses(&mut self) {
        // Take the use_decls out of the module tree so we can iterate
        // without borrowing self mutably.
        let use_decls = std::mem::take(&mut self.module_tree.use_decls);
        let interner_placeholder: Rodeo = Rodeo::new();
        for decl in &use_decls {
            self.resolve_use_tree(&decl.tree, &interner_placeholder);
        }
        // Restore use_decls (so future introspection works).
        self.module_tree.use_decls = use_decls;
        self.module_tree.uses_resolved = true;
    }

    /// Recursively resolve a `UseTree`. Leaf nodes register imports;
    /// Path nodes recurse into children; Glob nodes expand.
    fn resolve_use_tree(&mut self, tree: &HirUseTree, _interner: &Rodeo) {
        match tree {
            HirUseTree::Leaf(path, alias) => {
                self.resolve_use_leaf(path, alias.as_ref());
            }
            HirUseTree::Glob(path) => {
                self.resolve_use_glob(path);
            }
            HirUseTree::Path { prefix, children } => {
                // For `use a::{b, c};`, recurse into each child.
                // Note: we don't yet support `use a::b::{c, d};` (multi-level
                // path prefix) — Stage 4 work. For now, the prefix is
                // ignored and children are resolved at the crate root.
                let _ = prefix; // suppress unused warning
                for child in children {
                    self.resolve_use_tree(child, _interner);
                }
            }
        }
    }

    /// Resolve a leaf import: `use a::b::c;` or `use a::b::c as d;`.
    ///
    /// The last segment of `path` is the imported name (or `alias` if
    /// present). The preceding segments identify the target definition.
    /// For now, we only support single-segment targets (`use foo;`)
    /// and two-segment targets (`use mod::foo;`). Longer paths require
    /// cross-module resolution which is Stage 4+ work.
    fn resolve_use_leaf(&mut self, path: &HirPath, alias: Option<&crate::ast::Ident>) {
        if path.segments.is_empty() {
            self.errors.push(ResolveError::new(
                "use declaration with empty path",
                path.span,
            ));
            return;
        }

        // Determine the imported name (alias takes precedence).
        let imported_name = if let Some(alias) = alias {
            alias.name
        } else {
            path.segments.last().unwrap().ident.name
        };

        // Try to resolve the path to find the target DefId.
        // We use a simplified resolution: look up the last segment
        // in the module tree (single-segment) or walk the path
        // (multi-segment, limited to 2 levels for now).
        let target = self.lookup_use_path_target(path);

        match target {
            Some((def_id, kind)) => {
                let import = UseImport {
                    target: def_id,
                    kind,
                    is_glob: false,
                };
                if let Err(existing) = self.module_tree.insert_use_import(imported_name, import) {
                    self.errors.push(ResolveError::new(
                        format!(
                            "ambiguous import: `{}` is already imported (pointing to {:?})",
                            self.name_to_string(imported_name),
                            existing.target
                        ),
                        path.span,
                    ));
                }
            }
            None => {
                self.errors.push(ResolveError::new(
                    format!("unresolved import `{}`", self.path_to_string(path)),
                    path.span,
                ));
            }
        }
    }

    /// Resolve a glob import: `use a::b::*;`.
    ///
    /// All public items from the target module are registered as glob
    /// imports. Glob imports have lower priority than leaf imports.
    fn resolve_use_glob(&mut self, path: &HirPath) {
        // For `use mod::*;`, look up the module and copy all its
        // value_ns + type_ns entries into use_imports as globs.
        if path.segments.is_empty() {
            return;
        }

        let target_module_name = path.segments.last().unwrap().ident.name;

        // Try to find the target module.
        if let Some(child_mod) = self.module_tree.child(target_module_name) {
            // Clone the entries to avoid borrow conflict.
            let value_entries: Vec<(Spur, DefId)> =
                child_mod.value_ns.iter().map(|(k, v)| (*k, *v)).collect();
            let type_entries: Vec<(Spur, DefId)> =
                child_mod.type_ns.iter().map(|(k, v)| (*k, *v)).collect();

            for (name, def_id) in value_entries {
                let kind = self.def_kinds.get(&def_id).copied().unwrap_or(DefKind::Fn);
                let import = UseImport {
                    target: def_id,
                    kind,
                    is_glob: true,
                };
                // Ignore errors here — globs don't error on duplicates.
                let _ = self.module_tree.insert_use_import(name, import);
            }
            for (name, def_id) in type_entries {
                let kind = self
                    .def_kinds
                    .get(&def_id)
                    .copied()
                    .unwrap_or(DefKind::Struct);
                let import = UseImport {
                    target: def_id,
                    kind,
                    is_glob: true,
                };
                let _ = self.module_tree.insert_use_import(name, import);
            }
        }
        // If the target module isn't found, silently ignore (the path
        // resolver will report the error when the globbed name is used).
    }

    /// Look up the target of a use path. Returns (DefId, DefKind) on success.
    /// Currently supports:
    /// - Single-segment: `use foo;` → looks up `foo` in value or type namespace.
    /// - Two-segment: `use mod::foo;` → looks up `foo` in `mod`'s namespace.
    /// - Longer paths: not yet supported (returns None).
    fn lookup_use_path_target(&self, path: &HirPath) -> Option<(DefId, DefKind)> {
        match path.segments.len() {
            1 => {
                let name = path.segments[0].ident.name;
                // Try value namespace first, then type namespace.
                if let Some(def_id) = self.module_tree.lookup_value(name) {
                    let kind = self.def_kinds.get(&def_id).copied().unwrap_or(DefKind::Fn);
                    return Some((def_id, kind));
                }
                if let Some(def_id) = self.module_tree.lookup_type(name) {
                    let kind = self
                        .def_kinds
                        .get(&def_id)
                        .copied()
                        .unwrap_or(DefKind::Struct);
                    return Some((def_id, kind));
                }
                None
            }
            2 => {
                let mod_name = path.segments[0].ident.name;
                let item_name = path.segments[1].ident.name;
                if let Some(child_mod) = self.module_tree.child(mod_name) {
                    if let Some(def_id) = child_mod.lookup_value(item_name) {
                        let kind = self.def_kinds.get(&def_id).copied().unwrap_or(DefKind::Fn);
                        return Some((def_id, kind));
                    }
                    if let Some(def_id) = child_mod.lookup_type(item_name) {
                        let kind = self
                            .def_kinds
                            .get(&def_id)
                            .copied()
                            .unwrap_or(DefKind::Struct);
                        return Some((def_id, kind));
                    }
                }
                None
            }
            _ => {
                // Longer paths (3+ segments) not yet supported.
                None
            }
        }
    }

    /// Helper: format a Spur name for error messages.
    fn name_to_string(&self, name: Spur) -> String {
        // We don't have access to the interner here (resolve_uses doesn't
        // take it). Fall back to a placeholder.
        format!("symbol({:?})", name)
    }

    /// Helper: format a HirPath for error messages.
    fn path_to_string(&self, path: &HirPath) -> String {
        // Same limitation as name_to_string — no interner access.
        let segs: Vec<String> = path
            .segments
            .iter()
            .map(|s| format!("symbol({:?})", s.ident.name))
            .collect();
        segs.join("::")
    }

    // ================================================================
    // Phase 3: Resolve all HirPath nodes
    // ================================================================

    fn resolve_all_paths(&mut self, hir: &mut HirCrate, interner: &Rodeo) {
        // Walk all owners.
        for (_, node) in hir.owners.iter_mut() {
            self.resolve_owner_paths(node, interner);
        }
        // Walk all bodies.
        for (_, body) in hir.bodies.iter_mut() {
            self.resolve_body(body, interner);
        }
    }

    fn resolve_owner_paths(&mut self, node: &mut OwnerNode, interner: &Rodeo) {
        if let OwnerNode::Item(item) = node {
            self.resolve_item_paths(item, interner);
        }
    }

    fn resolve_item_paths(&mut self, item: &mut HirItem, interner: &Rodeo) {
        match item {
            HirItem::Fn(f) => {
                self.resolve_generics_paths(&mut f.generics, interner);
                for param in &mut f.sig.inputs {
                    if let Some(ty) = &mut param.ty {
                        self.resolve_ty_paths(ty, interner);
                    }
                }
                if let HirFnRetTy::Ty(ty) = &mut f.sig.output {
                    self.resolve_ty_paths(ty, interner);
                }
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
                self.resolve_generics_paths(&mut t.generics, interner);
                for bound in &mut t.supertraits {
                    if let HirTypeBound::Trait(tb) = bound {
                        self.resolve_hir_path(&mut tb.path, interner);
                    }
                }
            }
            HirItem::Impl(i) => {
                self.resolve_generics_paths(&mut i.generics, interner);
                self.resolve_ty_paths(&mut i.self_ty, interner);
                if let Some(trait_path) = &mut i.of_trait {
                    self.resolve_hir_path(trait_path, interner);
                }
            }
            HirItem::TypeAlias(t) => {
                self.resolve_generics_paths(&mut t.generics, interner);
                self.resolve_ty_paths(&mut t.ty, interner);
            }
            _ => {}
        }
    }

    fn resolve_generics_paths(&mut self, generics: &mut HirGenerics, interner: &Rodeo) {
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

    fn resolve_ty_paths(&mut self, ty: &mut HirTy, interner: &Rodeo) {
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

    fn resolve_hir_path(&mut self, path: &mut HirPath, interner: &Rodeo) {
        if path.res != Res::Unknown {
            return;
        }
        path.res = self.resolve_path(path, interner);
    }

    /// Core path resolution: look up a HirPath in the module tree + scope chain.
    fn resolve_path(&self, path: &HirPath, interner: &Rodeo) -> Res {
        if path.segments.is_empty() {
            return Res::Err;
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
            // from impl-Self. The resolver doesn't yet track owner context
            // (trait vs impl) — defaults to Impl. A follow-up round can
            // thread the owner kind through to make this precise.
            if let Some(self_spur) = interner.get("Self") {
                if seg.ident.name == self_spur {
                    return Res::SelfTy(crate::hir::HirSelfKind::Impl);
                }
            }
            if name == "Self" {
                return Res::SelfTy(crate::hir::HirSelfKind::Impl);
            }

            // Value namespace (fn, const, static).
            if let Some(def_id) = self.module_tree.lookup_value(seg.ident.name) {
                // Stage 3.30: look up DefKind from the def_kinds table so
                // downstream passes (MIR lower, codegen) can distinguish
                // fn calls from struct ctors without re-querying HIR.
                let kind = self.def_kinds.get(&def_id).copied().unwrap_or(DefKind::Fn);
                return Res::Def(def_id, kind);
            }

            // Type namespace (struct, enum, trait, type alias, mod).
            if let Some(def_id) = self.module_tree.lookup_type(seg.ident.name) {
                let kind = self
                    .def_kinds
                    .get(&def_id)
                    .copied()
                    .unwrap_or(DefKind::Struct);
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

    fn resolve_body(&mut self, body: &mut Body, interner: &Rodeo) {
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
    fn collect_pat_bindings(&mut self, pat: &mut HirPat, interner: &Rodeo) {
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

    fn resolve_expr(&mut self, expr: &mut HirExpr, interner: &Rodeo) {
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
            HirExprKind::Unsafe(b) => self.resolve_block(b, interner),
        }
    }

    fn resolve_block(&mut self, block: &mut HirBlock, interner: &Rodeo) {
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

    pub fn into_errors(self) -> Vec<ResolveError> {
        self.errors
    }
}

/// Look up a primitive type by name string.
fn lookup_prim_ty(name: &str) -> Option<PrimTy> {
    Some(match name {
        "bool" => PrimTy::Bool,
        "char" => PrimTy::Char,
        "i8" => PrimTy::I8,
        "i16" => PrimTy::I16,
        "i32" => PrimTy::I32,
        "i64" => PrimTy::I64,
        "i128" => PrimTy::I128,
        "isize" => PrimTy::Isize,
        "u8" => PrimTy::U8,
        "u16" => PrimTy::U16,
        "u32" => PrimTy::U32,
        "u64" => PrimTy::U64,
        "u128" => PrimTy::U128,
        "usize" => PrimTy::Usize,
        "f32" => PrimTy::F32,
        "f64" => PrimTy::F64,
        "str" => PrimTy::Str,
        _ => return None,
    })
}

/// Public entry point: resolve all names in the HIR crate.
/// Returns a list of resolution errors (non-fatal; the HIR is still
/// mutated with best-effort Res values).
///
/// Takes `&mut Rodeo` to pre-intern keyword strings ("Self", "self",
/// "crate", "super") that the parser looks up via `interner.get()`
/// but never interns itself (because the parser only has `&Rodeo`).
pub fn resolve_crate(hir: &mut HirCrate, interner: &mut Rodeo) -> Vec<ResolveError> {
    // Pre-intern keyword strings that the parser looks up but doesn't intern.
    // The parser's `ident_from_token` for KwSelfType/KwSelf_/KwCrate/KwSuper
    // calls `interner.get("Self")` etc. — if these strings haven't been
    // interned yet, the lookup returns None and the ident falls back to
    // Spur::default(), losing the keyword information.
    interner.get_or_intern("Self");
    interner.get_or_intern("self");
    interner.get_or_intern("crate");
    interner.get_or_intern("super");

    let mut resolver = Resolver::new();
    resolver.resolve(hir, interner);
    resolver.into_errors()
}
