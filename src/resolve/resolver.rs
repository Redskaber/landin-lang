//! Name resolver: walks HIR and fills `Res` on all `HirPath` nodes.
//!
//! Public entry point: [`resolve_crate`].
//!
//! ## Stage 6.16 architectural split (TD-026)
//!
//! Per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4 (aligned with
//! 01-language-specification.md §6.2 解析顺序), this file has been split
//! into 3 sub-modules:
//!
//! - `module_build.rs` — module tree building + use import resolution (§6.2 pass 1-3)
//! - `path_resolve.rs` — late resolve: path/expr/body resolution (§6.2 pass 4-5)
//! - `primitives.rs`   — primitive type lookup table
//!
//! This file (`resolver.rs`) retains: Resolver struct + `new` + `resolve`
//! orchestrator + `into_errors` + helper accessors + `resolve_crate` entry.

use crate::hir::*;
use crate::resolve::error::ResolveError;
use crate::resolve::module_tree::{DefKind, ModuleNode};
use crate::resolve::scope::ScopeStack;
use lasso::{Rodeo, Spur};
use std::collections::HashMap;

/// The name resolver. Holds the module tree, DefKind map, scope stack,
/// and errors.
#[derive(Default)]
pub struct Resolver {
    /// Module tree: crate root → nested mods.
    pub(super) module_tree: ModuleNode,
    /// Map from DefId → DefKind (for namespace disambiguation).
    pub(super) def_kinds: HashMap<DefId, DefKind>,
    /// Stage 3.68: Map from DefId → Visibility (for visibility checking).
    /// Populated during `build_module_tree`. Currently used for infrastructure
    /// only — the actual visibility check is a stub since the module tree is
    /// flat (all items at crate root). Once nested modules are supported
    /// (Stage 4), `check_visibility` will enforce access rules.
    pub(super) def_visibility: HashMap<DefId, crate::ast::Visibility>,
    /// Scope stack for local variable resolution (Stage 1.4).
    /// `None` when not inside a body (e.g., during module tree construction).
    pub(super) scopes: Option<ScopeStack>,
    /// Stage 3.66: Current owner context for `Self` resolution.
    /// `None` at crate level; `Some(Trait)` inside a trait declaration;
    /// `Some(Impl)` inside an impl block. Used by `resolve_path` to produce
    /// accurate `Res::SelfTy(HirSelfKind)`.
    pub(super) current_self_kind: Option<crate::hir::HirSelfKind>,
    /// Stage 4.12: Current module name for visibility enforcement.
    /// `None` at crate root; `Some(module_name)` when resolving inside a
    /// nested module. Used by `check_visibility` to determine if the caller
    /// is in the same module as the definition (private access) or a
    /// different module (requires pub).
    pub(super) current_module: Option<Spur>,
    /// Stage 14.41: Impl method index — maps `(type_name, method_name)` to
    /// the method's DefId. Used to resolve `Type::method` paths (e.g.,
    /// `Vec::new`, `Counter::create`) to the actual method, NOT to the
    /// struct itself.
    ///
    /// Per §13.4 + §16 (interface isolation): the index is built during
    /// `build_module_tree` (Phase 1) and read during `resolve_path` (Phase 3).
    /// This eliminates the long-standing bug where `V::new(1, 2)` was treated
    /// as a struct constructor `V { x: 1, y: 2 }` instead of calling the
    /// `new` method — the resolver now returns `Res::Def(method_def_id, Fn)`
    /// for `V::new`, and the MIR lower's `is_adt_ctor` check no longer
    /// misfires.
    pub(super) impl_method_index: HashMap<(Spur, Spur), DefId>,
    /// Stage 14.42: Set of impl method DefIds — used to skip registering
    /// impl methods in the top-level value namespace.
    ///
    /// Without this, two impl blocks with same-named methods (e.g.,
    /// `A::new` and `B::new`) would collide in the value namespace.
    /// Impl methods are accessed via `Type::method` paths (impl_method_index),
    /// NOT as free functions.
    pub(super) impl_method_def_ids: std::collections::HashSet<DefId>,
    /// Stage 18.54: Stack of generic type parameter scopes.
    ///
    /// Each entry is a list of `(name, index)` pairs for the current owner's
    /// generic type parameters. Pushed when entering fn/struct/enum/trait/impl
    /// signature resolution; popped on exit.
    ///
    /// Per §1.0 原則 6 "通用 > 特例": one stack handles all owner kinds —
    /// fn/struct/enum/trait/impl all use the same enter/exit/lookup API.
    /// Per §1.0 原則 3 "显式 > 隐式": generic params are explicitly tracked,
    /// not implicitly folded into module_tree or scopes.
    pub(super) generic_param_scope: Vec<Vec<(Spur, usize)>>,
    /// Errors encountered (non-fatal).
    pub(super) errors: Vec<ResolveError>,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            module_tree: ModuleNode::new(),
            def_kinds: HashMap::new(),
            def_visibility: HashMap::new(),
            scopes: None,
            current_self_kind: None,
            current_module: None,
            impl_method_index: HashMap::new(),
            impl_method_def_ids: std::collections::HashSet::new(),
            generic_param_scope: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Resolve all paths in the HIR crate, mutating `HirPath.res` in-place.
    pub fn resolve(&mut self, hir: &mut HirCrate, interner: &mut Rodeo) {
        self.build_module_tree(hir, interner);
        self.resolve_uses();
        self.resolve_all_paths(hir, interner);
    }

    // ================================================================
    // Phase 1: Build module tree
    // ================================================================
    //
    // Stage 4.1: now recursively processes nested inline modules.
    // Previously (Stage 1.3-3.68): all items were registered at the
    // crate root level — `ModuleNode.children` was never populated.
    // Now: when we encounter `HirItem::Mod` with `HirModKind::Inline(items)`,
    // we recurse into the items and register them in a child ModuleNode.

    /// Helper: format a Spur name for error messages.
    pub(super) fn name_to_string(&self, name: Spur) -> String {
        // We don't have access to the interner here (resolve_uses doesn't
        // take it). Fall back to a placeholder.
        format!("symbol({:?})", name)
    }

    /// Helper: format a HirPath for error messages.
    pub(super) fn path_to_string(&self, path: &HirPath) -> String {
        // Same limitation as name_to_string — no interner access.
        let segs: Vec<String> = path
            .segments
            .iter()
            .map(|s| format!("symbol({:?})", s.ident.name))
            .collect();
        segs.join("::")
    }

    // ================================================================
    // Stage 18.54: Generic type parameter scope management
    // ================================================================

    /// Stage 18.54: Enter a generic type parameter scope.
    ///
    /// Pushes a new scope frame containing all type parameters from the
    /// given `HirGenerics`. Called when entering fn/struct/enum/trait/impl
    /// signature resolution.
    ///
    /// Per §10 naming: `enter_generic_scope` follows `<verb>_<adj>_<noun>` pattern.
    /// Per §1.0 原則 6 "通用 > 特例": one method for all owner kinds.
    pub(super) fn enter_generic_scope(&mut self, generics: &crate::hir::HirGenerics) {
        let mut params: Vec<(Spur, usize)> = Vec::new();
        for (idx, param) in generics.params.iter().enumerate() {
            if let crate::hir::HirGenericParam::Type(tp) = param {
                params.push((tp.ident.name, idx));
            }
            // Lifetime params are not tracked here — they're erased in MIR
            // and don't participate in type resolution as values.
        }
        self.generic_param_scope.push(params);
    }

    /// Stage 18.54: Exit the current generic type parameter scope.
    ///
    /// Pops the top scope frame. Called after signature resolution completes.
    ///
    /// Per §10 naming: `exit_generic_scope` follows `<verb>_<adj>_<noun>` pattern.
    pub(super) fn exit_generic_scope(&mut self) {
        self.generic_param_scope.pop();
    }

    /// Stage 18.54: Look up a generic type parameter by name in the scope stack.
    ///
    /// Searches from innermost scope (top of stack) outward. Returns the
    /// parameter's index if found, `None` otherwise.
    ///
    /// Per §10 naming: `lookup_generic_param` follows `<verb>_<noun>_<noun>` pattern.
    pub(super) fn lookup_generic_param(&self, name: Spur) -> Option<usize> {
        // Search from innermost (top) to outermost (bottom).
        for scope in self.generic_param_scope.iter().rev() {
            for (param_name, idx) in scope.iter() {
                if *param_name == name {
                    return Some(*idx);
                }
            }
        }
        None
    }

    pub fn into_errors(self) -> Vec<ResolveError> {
        self.errors
    }

    /// Stage 3.68: Public accessor for the visibility map (for testing).
    pub fn def_visibility(&self, def_id: crate::hir::DefId) -> Option<&crate::ast::Visibility> {
        self.def_visibility.get(&def_id)
    }

    /// Stage 4.12: Public accessor for the current module (for testing).
    pub fn current_module(&self) -> Option<Spur> {
        self.current_module
    }
}

/// Public entry point: resolve all names in the HIR crate.
/// Returns a list of resolution errors (non-fatal; the HIR is still
/// mutated with best-effort Res values).
///
/// Stage 3.67: now takes `&Rodeo` (was `&mut Rodeo`). The lexer now
/// interns keyword strings ("Self", "self", "crate", "super") at
/// tokenization time, so the resolver no longer needs to pre-intern
/// them. This eliminates the `&mut Rodeo` smell — the resolver is now
/// a pure read-only consumer of the interner.
pub fn resolve_crate(hir: &mut HirCrate, interner: &mut Rodeo) -> Vec<ResolveError> {
    let mut resolver = Resolver::new();
    resolver.resolve(hir, interner);
    resolver.into_errors()
}
