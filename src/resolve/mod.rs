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

// Stage 6.16 (TD-026) sub-modules of `resolver` — declared here so they
// resolve to `src/resolve/{module_build,path_resolve,primitives}.rs`
// (sibling to `resolver.rs`). Each sub-module adds methods to `impl Resolver`
// via its own `impl` block, per §14.4 (refactoring as architecture design)
// aligned with 01-language-specification.md §6.2 解析顺序.
mod module_build;
mod path_resolve;
mod primitives;

pub use error::{ResolveError, ResolveErrorKind};
pub use module_tree::{ModuleNode, UseDecl, UseImport};
pub use resolver::{resolve_crate, Resolver};
pub use scope::{Scope, ScopeKind, ScopeStack};
// Stage 3.63: `DefKind` is now defined in `crate::hir::kinds` and re-exported
// from `crate::hir`. Re-export here too for backwards compatibility with
// callers that historically used `crate::resolve::DefKind`.
pub use crate::hir::DefKind;
