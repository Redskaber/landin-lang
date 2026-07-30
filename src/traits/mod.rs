//! Trait resolution: collect trait definitions + impl blocks + build dispatch tables.
//!
//! Stage 5.23: split into sub-modules per deep review r70 (TD-NEW-1):
//! - `vtable.rs` — VtableEntry + Vtable structs
//! - `builtin.rs` — BUILTIN_TRAIT_NAMES + constants + is_primitive_copy_kind
//! - `resolver.rs` — TraitResolver + TraitInfo + ImplInfo + error types + all methods
//!
//! This file re-exports all public items so external callers see no change.
//!
//! Per §16: TraitResolver reads HIR during the driver's pre-computation phase,
//! then provides data to typeck/borrowck/codegen.

pub mod builtin;
// Stage 14.105 (dead code cleanup): `object_safety` module removed.
// It was `#[allow(dead_code)]` since Stage 8.2 and never called.
// Object safety will be re-implemented in v0.2 when dyn Trait is fully supported.
pub mod resolver;
pub mod vtable;

pub use builtin::{
    is_primitive_copy_kind, BUILTIN_DEF_ID_BASE, BUILTIN_PRIMITIVE_COPY_KINDS, BUILTIN_TRAIT_NAMES,
};
pub use resolver::{
    extract_impl_self_ty_name, CoherenceError, ImplInfo, ImplValidationReport, IncompleteImpl,
    TraitInfo, TraitResolver,
};
pub use vtable::{Vtable, VtableEntry};
