//! Name resolution: HIR path resolution + module tree.
//!
//! Per Stage 1.3 plan: walk all `HirPath` nodes and replace `Res::Unknown`
//! with the appropriate `Res::{Def, Local, PrimTy, SelfTy, ...}`.
//!
//! Stage 1.3 handles **module-level** resolution only (items, use imports,
//! path resolution, primitives, Self). Local variable resolution is Stage 1.4.

pub mod error;
pub mod module_tree;
pub mod resolver;
pub mod scope;

pub use error::ResolveError;
pub use module_tree::ModuleNode;
pub use resolver::{resolve_crate, Resolver};
pub use scope::{Scope, ScopeKind, ScopeStack};
// Stage 3.63: `DefKind` is now defined in `crate::hir::kinds` and re-exported
// from `crate::hir`. Re-export here too for backwards compatibility with
// callers that historically used `crate::resolve::DefKind`.
pub use crate::hir::DefKind;
