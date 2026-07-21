//! Module tree: hierarchical namespace structure for name resolution.
//!
//! Per 02-grammar.md §3.7 + 06-mir.md §4: Landin has two namespaces:
//! - **Value namespace**: fn, const, static
//! - **Type namespace**: struct, enum, trait, type alias, mod
//!
//! Each module has both namespaces. Child modules are keyed by name.

use crate::ast::Visibility;
use crate::hir::DefId;
// Stage 3.63 (cross-stage naming standardization): `DefKind` is now
// imported from `crate::hir::DefKind` (its architectural home). The
// former local definition has been removed — DRY restored.
pub use crate::hir::DefKind;
use lasso::Spur;
use std::collections::HashMap;

/// A pending use declaration awaiting resolution.
#[derive(Debug, Clone)]
pub struct UseDecl {
    /// The use tree to resolve.
    pub tree: crate::hir::HirUseTree,
    /// Visibility of this use.
    pub vis: Visibility,
    /// The span of the `use` keyword (for error reporting).
    pub span: crate::session::Span,
}

/// A resolved use import (Stage 3.64).
///
/// After `resolve_uses` runs, every `use a::b::c;` (leaf) or
/// `use a::b::*;` (glob) declaration is registered here so that
/// `resolve_path` can find imported names.
#[derive(Debug, Clone, Copy)]
pub struct UseImport {
    /// The DefId that this import points to.
    pub target: DefId,
    /// The DefKind of the target (Fn/Const/Static/Struct/Enum/Trait/TypeAlias/Mod).
    pub kind: DefKind,
    /// Whether this is a glob import (`use a::b::*;`). Glob imports
    /// are lower priority than explicit leaf imports — a leaf import
    /// shadows a glob import with the same name.
    pub is_glob: bool,
}

/// A node in the module tree. Represents one module (the crate root
/// or a `mod foo { ... }` block).
#[derive(Debug, Clone, Default)]
pub struct ModuleNode {
    /// Value namespace: fn, const, static — keyed by symbol.
    pub value_ns: HashMap<Spur, DefId>,
    /// Type namespace: struct, enum, trait, type alias, mod — keyed by symbol.
    pub type_ns: HashMap<Spur, DefId>,
    /// Child modules, keyed by module name.
    pub children: HashMap<Spur, ModuleNode>,
    /// All `use` declarations in this module (processed during resolution).
    pub use_decls: Vec<UseDecl>,
    /// Whether this module's use declarations have been processed.
    pub uses_resolved: bool,
    /// Resolved use imports (Stage 3.64). Keyed by the imported name.
    /// Both value-namespace and type-namespace imports land here —
    /// the `kind` field disambiguates.
    pub use_imports: HashMap<Spur, UseImport>,
}

impl ModuleNode {
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a name in the value namespace.
    pub fn lookup_value(&self, name: Spur) -> Option<DefId> {
        self.value_ns.get(&name).copied()
    }

    /// Look up a name in the type namespace.
    pub fn lookup_type(&self, name: Spur) -> Option<DefId> {
        self.type_ns.get(&name).copied()
    }

    /// Stage 3.64: Look up a name in the use-imports table.
    /// Returns the resolved `UseImport` if found. Callers should
    /// prefer leaf imports (is_glob=false) over glob imports when
    /// both exist for the same name.
    pub fn lookup_use_import(&self, name: Spur) -> Option<UseImport> {
        self.use_imports.get(&name).copied()
    }

    /// Insert a definition into the appropriate namespace.
    /// Returns `Err(existing_def_id)` if the name is already taken.
    pub fn insert(&mut self, name: Spur, def_id: DefId, kind: DefKind) -> Result<(), DefId> {
        if kind.is_value() {
            if let Some(existing) = self.value_ns.get(&name) {
                return Err(*existing);
            }
            self.value_ns.insert(name, def_id);
        } else if kind.is_type() {
            if let Some(existing) = self.type_ns.get(&name) {
                return Err(*existing);
            }
            self.type_ns.insert(name, def_id);
        }
        // Some definitions (impl) are in neither namespace — they're
        // looked up via the self_ty + trait path. Skip insertion.
        Ok(())
    }

    /// Stage 3.64: Insert a use import. Leaf imports (is_glob=false)
    /// overwrite glob imports; glob imports don't overwrite leaf imports.
    /// Two leaf imports with the same name produce an ambiguity error
    /// (handled by the caller via `insert_use_import_ambiguity`).
    pub fn insert_use_import(&mut self, name: Spur, import: UseImport) -> Result<(), UseImport> {
        match self.use_imports.get(&name) {
            // No existing import — insert.
            None => {
                self.use_imports.insert(name, import);
                Ok(())
            }
            // Existing leaf import — error (caller will report ambiguity).
            Some(existing) if !existing.is_glob => Err(*existing),
            // Existing glob import — new import wins if it's a leaf.
            Some(_) => {
                if !import.is_glob {
                    self.use_imports.insert(name, import);
                    Ok(())
                } else {
                    // Two globs — keep the first, but don't error (globs
                    // are often re-exports; ambiguity is detected at
                    // use-site, not at import-site).
                    Ok(())
                }
            }
        }
    }

    /// Get a child module by name.
    pub fn child(&self, name: Spur) -> Option<&ModuleNode> {
        self.children.get(&name)
    }

    /// Get a mutable child module by name.
    pub fn child_mut(&mut self, name: Spur) -> Option<&mut ModuleNode> {
        self.children.get_mut(&name)
    }
}
