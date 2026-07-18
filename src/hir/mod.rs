//! High-level Intermediate Representation (HIR).
//!
//! Per 06-mir.md §3, HIR is the "high-level IR" — the AST plus per-node
//! `HirId`s and resolution placeholders. It is the input to:
//! - Stage 1.2: AST → HIR lowering (constructs the HIR from AST)
//! - Stage 1.3: Name resolution (populates `Res` in `HirPath`)
//! - Stage 1.4: Scope-based name resolution (locals within bodies)
//! - Stage 2: Type inference (populates `InferTy` in `HirTy`)
//! - Stage 2: Borrow check (uses `HirId` to key borrow info)
//!
//! Stage 1.1 (this module) only DEFINES the data structures; lowering and
//! resolution are implemented in subsequent sub-stages.

pub mod id;
pub mod kinds;
pub mod map;

// Re-export the most-used types at the module root for convenience.
pub use id::{DefId, DefIdCounter, HirId, ItemLocalId, ItemLocalIdCounter, OwnerId};
pub use kinds::*;
pub use map::{DefIdMap, DefIdSet, HirIdMap, HirIdSet};
