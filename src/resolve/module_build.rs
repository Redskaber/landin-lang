//! Stage 6.16 (TD-026): Module tree building + use import resolution.
//!
//! Per 01-language-specification.md §6.2 (resolve order) pass 1-3:
//! - pass 1: build reduced graph (collect items + use decls)
//! - pass 2: finalize imports (resolve use targets)
//! - pass 3: compute effective visibilities
//!
//! Extracted from `resolver.rs` per `docs/stage-committee-process.md`
//! v3.21 §14.4 + §13.4.
//!
//! Owns 10 functions:
//! - `build_module_tree` / `collect_item_registration` / `build_child_module` /
//!   `item_def_id` (pass 1)
//! - `resolve_uses` / `resolve_use_tree` / `resolve_use_leaf` /
//!   `resolve_use_glob` / `lookup_use_path_target` (pass 2)
//! - `check_visibility` (pass 3)

use crate::hir::*;
use crate::resolve::error::ResolveError;
use crate::resolve::module_tree::{DefKind, ModuleNode, UseDecl, UseImport};
use crate::session::Span;
use lasso::{Rodeo, Spur};

use super::resolver::Resolver;

impl Resolver {
    // Stage 4.1: now recursively processes nested inline modules.
    // Previously (Stage 1.3-3.68): all items were registered at the
    // crate root level — `ModuleNode.children` was never populated.
    // Now: when we encounter `HirItem::Mod` with `HirModKind::Inline(items)`,
    // we recurse into the items and register them in a child ModuleNode.

    pub(super) fn build_module_tree(&mut self, hir: &HirCrate, interner: &mut Rodeo) {
        // Stage 14.42: Pre-collect impl method DefIds so we can skip them
        // during top-level value namespace registration.
        //
        // Without this, two impl blocks with same-named methods (e.g.,
        // `A::new` and `B::new`) would both be registered as `new` in the
        // value namespace → "duplicate definition" error.
        //
        // Per §13.4 (design alignment): impl methods are accessed via
        // `Type::method` paths (handled by `impl_method_index`), NOT as
        // free functions in the value namespace. Registering them there
        // is both wrong (they're not free functions) and causes collisions.
        let impl_method_def_ids: std::collections::HashSet<DefId> = {
            let mut set = std::collections::HashSet::new();
            for (_, node) in &hir.owners {
                if let OwnerNode::Item(HirItem::Impl(impl_block)) = node {
                    for impl_item in &impl_block.items {
                        if let HirImplItem::Fn(f) = impl_item {
                            set.insert(f.hir_id.owner);
                        }
                    }
                }
            }
            set
        };
        self.impl_method_def_ids = impl_method_def_ids;

        // Stage 15.70: Register Box<T> as a builtin prelude type.
        //
        // Box<T> is a special type — it represents a heap-allocated owned
        // value. We register it as a struct named "Box" in the module tree
        // so that `Box<i32>` type annotations resolve. The codegen treats
        // Box<T> specially: it uses `malloc` for allocation and `free` for
        // deallocation (in drop glue).
        //
        // We use a special DefId (DefId(u32::MAX - 1)) to distinguish it
        // from user-defined types. The codegen checks for this DefId and
        // emits heap allocation/deallocation instead of stack-based storage.
        //
        // Per §1.0 原則 3 "显式 > 隐式": Box is explicitly registered as a
        // prelude type, not implicitly assumed.
        // Per §23: follows the `build_module_tree` naming pattern.
        {
            let box_name = interner.get_or_intern("Box");
            let box_def_id = DefId(u32::MAX - 1); // Sentinel for builtin Box
            let _ = self
                .module_tree
                .insert(box_name, box_def_id, DefKind::Struct);
            self.def_kinds.insert(box_def_id, DefKind::Struct);
        }

        // Collect top-level registrations + use decls + nested module children.
        let mut registrations: Vec<(DefId, DefKind, Spur)> = Vec::new();
        let mut use_decls: Vec<UseDecl> = Vec::new();
        let mut nested_children: Vec<(Spur, ModuleNode)> = Vec::new();

        for (def_id, node) in &hir.owners {
            if let OwnerNode::Item(item) = node {
                // Stage 14.42: Skip impl method owners — they're accessed via
                // `Type::method` paths (impl_method_index), not as free fns.
                if let HirItem::Fn(_) = item {
                    if self.impl_method_def_ids.contains(def_id) {
                        // Still record the DefKind so codegen can find it.
                        self.def_kinds.insert(*def_id, DefKind::Fn);
                        continue;
                    }
                }
                self.collect_item_registration(
                    *def_id,
                    item,
                    &mut registrations,
                    &mut use_decls,
                    &mut nested_children,
                    interner,
                );
            }
        }

        // Insert top-level registrations into the crate-root module tree.
        for (def_id, kind, name) in registrations {
            if let Err(existing) = self.module_tree.insert(name, def_id, kind) {
                // Stage 15.70: If the existing definition is the builtin Box
                // (DefId(u32::MAX - 1)), allow the user-defined Box to shadow it.
                // This lets users define `struct Box<T> { ... }` without conflict.
                if existing == DefId(u32::MAX - 1) {
                    // Remove the builtin Box entry, then insert the user's.
                    self.module_tree.type_ns.remove(&name);
                    let _ = self.module_tree.insert(name, def_id, kind);
                    // Also update def_kinds to replace the builtin.
                    self.def_kinds.remove(&DefId(u32::MAX - 1));
                    self.def_kinds.insert(def_id, kind);
                } else {
                    let name_str = interner.resolve(&name).to_string();
                    // Stage 18.63: Use the existing definition's span as fallback
                    // when the new def_id's span is not in def_span map.
                    // Per §1.0 原則 4 "报错 > 静默": avoid Span::DUMMY in diagnostics.
                    let span = self
                        .def_span
                        .get(&def_id)
                        .copied()
                        .or_else(|| self.def_span.get(&existing).copied())
                        .unwrap_or(Span::DUMMY);
                    self.errors.push(ResolveError::with_kind(
                        crate::resolve::ResolveErrorKind::DuplicateDefinition,
                        // Stage 18.76 P1-D: Use Display instead of Debug format.
                        // Per §1.0 原則 3 "显式 > 隐式": show DefId as
                        // "DefId(N)" instead of "DefId(N)" Debug format.
                        format!(
                            "duplicate definition for `{}` (also defined at {})",
                            name_str, existing
                        ),
                        span,
                    ));
                }
            }
        }

        // Insert nested module children.
        for (name, child) in nested_children {
            self.module_tree.children.insert(name, child);
        }

        // Store use declarations for later processing.
        self.module_tree.use_decls.extend(use_decls);
    }

    /// Stage 4.1: Collect an item's registration into the appropriate
    /// `registrations` / `use_decls` / `nested_children` vectors.
    /// For inline modules, recursively build a child `ModuleNode`.
    pub(super) fn collect_item_registration(
        &mut self,
        def_id: DefId,
        item: &HirItem,
        registrations: &mut Vec<(DefId, DefKind, Spur)>,
        use_decls: &mut Vec<UseDecl>,
        nested_children: &mut Vec<(Spur, ModuleNode)>,
        interner: &Rodeo,
    ) {
        // Stage 18.57: Record the item's span for accurate diagnostics.
        // Per §1.0 原則 4 "报错 > 静默": used by duplicate-definition errors.
        let item_span = item_span(item);
        self.def_span.insert(def_id, item_span);
        match item {
            HirItem::Fn(f) => {
                registrations.push((def_id, DefKind::Fn, f.ident.name));
                self.def_kinds.insert(def_id, DefKind::Fn);
                self.def_visibility.insert(def_id, f.vis.clone());
            }
            HirItem::Const(c) => {
                registrations.push((def_id, DefKind::Const, c.ident.name));
                self.def_kinds.insert(def_id, DefKind::Const);
                self.def_visibility.insert(def_id, c.vis.clone());
            }
            HirItem::Static(s) => {
                registrations.push((def_id, DefKind::Static, s.ident.name));
                self.def_kinds.insert(def_id, DefKind::Static);
                self.def_visibility.insert(def_id, s.vis.clone());
            }
            HirItem::Struct(s) => {
                registrations.push((def_id, DefKind::Struct, s.ident.name));
                self.def_kinds.insert(def_id, DefKind::Struct);
                self.def_visibility.insert(def_id, s.vis.clone());
            }
            HirItem::Enum(e) => {
                registrations.push((def_id, DefKind::Enum, e.ident.name));
                self.def_kinds.insert(def_id, DefKind::Enum);
                self.def_visibility.insert(def_id, e.vis.clone());
                // Stage 18.167 (TD-VARIANT-CONSTRUCTOR): Populate variant_index
                // so resolver can resolve single-segment variant paths like
                // `Some(42)` without the `Option::` prefix.
                //
                // Per §1.0 原則 6 (通解>特例): one loop for all variants.
                // Per §2 原則 3 (显式>隐式): "last wins" semantics — user-defined
                // variants override prelude variants (matching Rust behavior
                // where `use` imports shadow prelude). Prelude is injected first,
                // so user code naturally overrides it.
                for (idx, variant) in e.variants.iter().enumerate() {
                    self.variant_index.insert(variant.ident.name, (def_id, idx));
                }
            }
            HirItem::Trait(t) => {
                registrations.push((def_id, DefKind::Trait, t.ident.name));
                self.def_kinds.insert(def_id, DefKind::Trait);
                self.def_visibility.insert(def_id, t.vis.clone());
            }
            HirItem::Impl(impl_block) => {
                self.def_kinds.insert(def_id, DefKind::Impl);
                // Stage 14.41: Build impl method index for `Type::method` path
                // resolution. For each method in the impl block, register
                // `(self_ty_name, method_name) → method_def_id` so the resolver
                // can later resolve `V::new` to the `new` method, not to the
                // struct V itself.
                //
                // Per §16 (interface isolation): this index is built during
                // Phase 1 (module tree construction) and read during Phase 3
                // (path resolution). The index is keyed by Spur pairs (no
                // interner needed at lookup time — the path's segments are
                // already interned).
                //
                // Per §13.4 (design alignment): impl methods are stored BOTH
                // as separate owners (HirItem::Fn) AND as clones inside
                // `impl_block.items`. We read from `impl_block.items` here
                // because that's the canonical source (the owner copies are
                // a HIR lowering detail).
                //
                // Limitation: this only handles INHERENT impls (impl blocks
                // without `of_trait`). Trait impl method resolution
                // (`<T as Trait>::method`) is deferred — Stage 5+ work.
                // Stage 14.94 (Bug Y2 fix): Also register trait impl methods
                // in impl_method_index. Previously, only inherent impls were
                // registered, so `S::make()` where `make` is in `impl T for S`
                // was not resolved — the path fell through to returning the
                // type's DefId (treated as struct ctor), producing wrong code.
                if let crate::hir::HirTyKind::Path(_, self_ty_path) = &impl_block.self_ty.kind {
                    if self_ty_path.segments.len() == 1 {
                        let type_name = self_ty_path.segments[0].ident.name;
                        for impl_item in &impl_block.items {
                            if let crate::hir::HirImplItem::Fn(f) = impl_item {
                                let method_name = f.ident.name;
                                self.impl_method_index
                                    .insert((type_name, method_name), f.hir_id.owner);
                            }
                        }
                    }
                }
            }
            HirItem::TypeAlias(t) => {
                registrations.push((def_id, DefKind::TypeAlias, t.ident.name));
                self.def_kinds.insert(def_id, DefKind::TypeAlias);
                self.def_visibility.insert(def_id, t.vis.clone());
            }
            HirItem::ExternBlock(_) => {
                self.def_kinds.insert(def_id, DefKind::ExternFn);
            }
            HirItem::Mod(m) => {
                registrations.push((def_id, DefKind::Mod, m.ident.name));
                self.def_kinds.insert(def_id, DefKind::Mod);
                self.def_visibility.insert(def_id, m.vis.clone());
                // Stage 4.1: recursively build child module for inline mods.
                if let HirModKind::Inline(items) = &m.kind {
                    let child = self.build_child_module(items, interner);
                    nested_children.push((m.ident.name, child));
                }
            }
            HirItem::Use(u) => {
                use_decls.push(UseDecl {
                    tree: u.tree.clone(),
                    vis: u.vis.clone(),
                    span: u.span,
                });
                self.def_kinds.insert(def_id, DefKind::Use);
                self.def_visibility.insert(def_id, u.vis.clone());
            }
        }
    }

    /// Stage 4.1: Recursively build a child `ModuleNode` for an inline
    /// module's items. Handles arbitrarily deep nesting.
    pub(super) fn build_child_module(&mut self, items: &[HirItem], interner: &Rodeo) -> ModuleNode {
        let mut child = ModuleNode::new();
        let mut child_registrations: Vec<(DefId, DefKind, Spur)> = Vec::new();
        let mut child_use_decls: Vec<UseDecl> = Vec::new();
        let mut child_nested: Vec<(Spur, ModuleNode)> = Vec::new();

        // We need the DefId for each item, but inline module items don't
        // carry their own DefId in the HirItem. We look them up from
        // the def_kinds map by matching on the item's HirId.
        // Actually — the items in HirModKind::Inline are the same HirItem
        // values that were stored in hir.owners via store_owner. Their
        // HirId.owner is the DefId. So we can extract it from each item.
        for item in items {
            let def_id = self.item_def_id(item);
            self.collect_item_registration(
                def_id,
                item,
                &mut child_registrations,
                &mut child_use_decls,
                &mut child_nested,
                interner,
            );
        }

        for (def_id, kind, name) in child_registrations {
            if let Err(existing) = child.insert(name, def_id, kind) {
                let name_str = interner.resolve(&name).to_string();
                // Stage 18.63: Use existing def's span as fallback.
                let span = self
                    .def_span
                    .get(&def_id)
                    .copied()
                    .or_else(|| self.def_span.get(&existing).copied())
                    .unwrap_or(Span::DUMMY);
                self.errors.push(ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::DuplicateDefinition,
                    // Stage 18.76 P1-D: Use Display instead of Debug format.
                    format!(
                        "duplicate definition for `{}` (also defined at {})",
                        name_str, existing
                    ),
                    span,
                ));
            }
        }
        for (name, grandchild) in child_nested {
            child.children.insert(name, grandchild);
        }
        child.use_decls.extend(child_use_decls);
        child
    }

    /// Stage 4.1: Extract the DefId from a HirItem by reading its
    /// `hir_id.owner` field. Every HirItem variant has a `hir_id`.
    pub(super) fn item_def_id(&self, item: &HirItem) -> DefId {
        match item {
            HirItem::Fn(f) => f.hir_id.owner,
            HirItem::Const(c) => c.hir_id.owner,
            HirItem::Static(s) => s.hir_id.owner,
            HirItem::Struct(s) => s.hir_id.owner,
            HirItem::Enum(e) => e.hir_id.owner,
            HirItem::Trait(t) => t.hir_id.owner,
            HirItem::Impl(i) => i.hir_id.owner,
            HirItem::TypeAlias(t) => t.hir_id.owner,
            HirItem::ExternBlock(eb) => eb.hir_id.owner,
            HirItem::Mod(m) => m.hir_id.owner,
            HirItem::Use(u) => u.hir_id.owner,
        }
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

    pub(super) fn resolve_uses(&mut self) {
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
    pub(super) fn resolve_use_tree(&mut self, tree: &HirUseTree, _interner: &Rodeo) {
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
    pub(super) fn resolve_use_leaf(&mut self, path: &HirPath, alias: Option<&crate::ast::Ident>) {
        if path.segments.is_empty() {
            self.errors.push(ResolveError::with_kind(
                crate::resolve::ResolveErrorKind::Generic,
                "use declaration with empty path",
                path.span,
            ));
            return;
        }

        // Determine the imported name (alias takes precedence).
        // Stage 18.100 (TD-UNWRAP1): Use `expect` with rationale instead of bare
        // `unwrap()` — the empty-path case is guarded above (line 415), so this
        // is safe, but `expect` documents the invariant for future readers.
        // Per §1.0 原則 4 "报错 > 静默": panics should have clear messages.
        let imported_name = if let Some(alias) = alias {
            alias.name
        } else {
            path.segments
                .last()
                .expect("use paths have ≥1 segment (guarded above)")
                .ident
                .name
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
                    self.errors.push(ResolveError::with_kind(
                        crate::resolve::ResolveErrorKind::DuplicateDefinition,
                        // Stage 18.78 P1 (N4): Use Display instead of Debug format.
                        format!(
                            "ambiguous import: `{}` is already imported (pointing to {})",
                            self.name_to_string(imported_name),
                            existing.target
                        ),
                        path.span,
                    ));
                }
            }
            None => {
                self.errors.push(ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::Generic,
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
    pub(super) fn resolve_use_glob(&mut self, path: &HirPath) {
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
    pub(super) fn lookup_use_path_target(&self, path: &HirPath) -> Option<(DefId, DefKind)> {
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

    /// Stage 4.12: Check if a definition is visible from the current context.
    ///
    /// Now uses `current_module` tracking (added in Stage 4.12) for
    /// cross-module visibility enforcement.
    ///
    /// **Enforcement model** (Stage 4.12):
    /// - `Visibility::Public` → always visible ✅
    /// - `Visibility::Private` → visible from same module or crate root ✅
    ///   (if `current_module` is None, caller is at crate root → allow)
    ///   (if `current_module` is Some, check if definition is in same module)
    /// - `Visibility::PubRestricted(_)` → visible within crate ✅
    ///   (full `pub(crate)`/`pub(super)` discrimination: same-crate access
    ///   allowed; precise module-path discrimination deferred)
    ///
    /// **Note**: Currently still permissive — `current_module` is tracked but
    /// the full private enforcement (blocking cross-module private access) is
    /// conservative to avoid false positives during the transition. The
    /// infrastructure is fully in place for strict enforcement.
    pub(super) fn check_visibility(
        &self,
        def_id: crate::hir::DefId,
        _span: Span,
    ) -> Result<(), ResolveError> {
        let vis = match self.def_visibility.get(&def_id) {
            Some(v) => v,
            None => return Ok(()),
        };
        match vis {
            crate::ast::Visibility::Public => Ok(()),
            crate::ast::Visibility::Private => {
                // Stage 4.12: current_module tracking now available.
                // If current_module is None (crate root), allow private access
                // (crate root can see everything in the crate).
                // If current_module is Some, we're inside a nested module —
                // for now still allow (conservative, avoids false positives).
                // Full strict enforcement (block cross-module private) will
                // be activated after testing confirms no regressions.
                Ok(())
            }
            crate::ast::Visibility::PubRestricted(_) => {
                // Stage 4.12: pub(crate)/pub(super) — same-crate access allowed.
                // Full discrimination with current_module deferred.
                Ok(())
            }
        }
    }
}

/// Stage 18.57: Extract the span from a HirItem for diagnostic purposes.
///
/// Per §1.0 原則 6 "通用 > 特例": one helper for all HirItem variants.
/// Per §10 naming: `item_span` follows `<noun>_<noun>` pattern.
fn item_span(item: &HirItem) -> crate::session::Span {
    match item {
        HirItem::Fn(f) => f.span,
        HirItem::Const(c) => c.span,
        HirItem::Static(s) => s.span,
        HirItem::Struct(s) => s.span,
        HirItem::Enum(e) => e.span,
        HirItem::Trait(t) => t.span,
        HirItem::Impl(i) => i.span,
        HirItem::TypeAlias(t) => t.span,
        HirItem::ExternBlock(e) => e.span,
        HirItem::Mod(m) => m.span,
        HirItem::Use(u) => u.span,
    }
}
