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
// Stage 18.95: TraitError moved from driver.rs to traits/error.rs.
pub mod error;
// Stage 16.64 (Task 14 Phase 1): Object safety checking re-implemented.
// Checks whether a trait is object-safe (whether `dyn Trait` can be used).
pub mod object_safety;
pub mod resolver;
pub mod vtable;

pub use builtin::{
    is_primitive_copy_kind, BUILTIN_DEF_ID_BASE, BUILTIN_PRIMITIVE_COPY_KINDS, BUILTIN_TRAIT_NAMES,
};
pub use error::TraitError;
pub use object_safety::{check_trait_object_safety, ObjectSafetyViolation};
pub use resolver::{
    extract_impl_self_ty_name, CoherenceError, ImplInfo, ImplValidationReport, IncompleteImpl,
    InherentImplConflict, PrimitiveInherentImplError, TraitInfo, TraitResolver,
};
pub use vtable::{Vtable, VtableEntry};
