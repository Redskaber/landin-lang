//! Module tree: hierarchical namespace structure for name resolution.
//!
//! Per 02-grammar.md §3.7 + 06-mir.md §4: Landin has two namespaces:
//! - **Value namespace**: fn, const, static
//! - **Type namespace**: struct, enum, trait, type alias, mod
//!
//! Each module has both namespaces. Child modules are keyed by name.

use crate::ast::Visibility;
use crate::hir::DefId;
use lasso::Spur;
use std::collections::HashMap;

/// The kind of a definition. Used for namespace disambiguation during
/// path resolution (e.g., `Foo` could be a struct type or a struct
/// constructor function — the DefKind tells us which).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    Fn,
    Const,
    Static,
    Struct,
    Enum,
    Trait,
    Impl,
    TypeAlias,
    Mod,
    Use,
    ExternFn,
    ExternStatic,
    ExternType,
}

impl DefKind {
    /// Returns `true` if this definition lives in the value namespace
    /// (fn, const, static, extern fn, extern static).
    pub fn is_value(self) -> bool {
        matches!(
            self,
            DefKind::Fn
                | DefKind::Const
                | DefKind::Static
                | DefKind::ExternFn
                | DefKind::ExternStatic
        )
    }

    /// Returns `true` if this definition lives in the type namespace
    /// (struct, enum, trait, type alias, mod, extern type).
    pub fn is_type(self) -> bool {
        matches!(
            self,
            DefKind::Struct
                | DefKind::Enum
                | DefKind::Trait
                | DefKind::TypeAlias
                | DefKind::Mod
                | DefKind::ExternType
        )
    }
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
}

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

    /// Get a child module by name.
    pub fn child(&self, name: Spur) -> Option<&ModuleNode> {
        self.children.get(&name)
    }

    /// Get a mutable child module by name.
    pub fn child_mut(&mut self, name: Spur) -> Option<&mut ModuleNode> {
        self.children.get_mut(&name)
    }
}
