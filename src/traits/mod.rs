//! Trait resolution: collect trait definitions + impl blocks + build dispatch tables.
//!
//! Stage 5.23: split into sub-modules per deep review r70 (TD-NEW-1):
//! - `vtable.rs` — VtableEntry + Vtable structs
//! - `builtin.rs` — BUILTIN_TRAIT_NAMES + constants + is_primitive_copy_kind
//! - `resolver.rs` — TraitResolver + TraitInfo + ImplInfo + error types + all methods
//!
//! Stage 19.1 (v0.5 Phase 1): added `solver` sub-module — Trait Solver
//! data structures (TraitPredicate + Goal + InferCtxt + ObligationQueue).
//! Phase 1 only declares structures; Phase 2+ adds the algorithm.
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
// Stage 18.308 (P3 LOC refactor): query/diagnostic methods extracted from
// resolver.rs per §13.4 J1-J6. The impl TraitResolver block lives here.
pub mod resolver_queries;
// Stage 19.1 (v0.5 Phase 1): Trait Solver data structures —
// TraitPredicate, Goal, InferCtxt, ObligationQueue, EvalResult,
// SelectionResult. Phase 1 only declares structures (no algorithm).
// Phase 2+ (Stage 19.2+) adds Evaluation, Selection, Fulfillment.
pub mod solver;
pub mod vtable;

pub use builtin::{
    is_primitive_copy_kind, BUILTIN_DEF_ID_BASE, BUILTIN_PRIMITIVE_COPY_KINDS, BUILTIN_TRAIT_NAMES,
};
pub use error::TraitError;
pub use object_safety::{check_trait_object_safety, ObjectSafetyViolation};
pub use resolver::{
    extract_impl_self_ty_name, CoherenceError, ImplInfo, ImplValidationReport, ImplWhereClause,
    IncompleteImpl, InherentImplConflict, OrphanRuleError, PrimitiveInherentImplError, TraitInfo,
    TraitResolver,
};
pub use solver::{
    Binder, EvalError, EvalResult, Goal, InferCtxt, InferCtxtError, Obligation, ObligationCause,
    ObligationQueue, ParamEnv, SelectionResult, TraitPredicate, Universe,
};
pub use vtable::{Vtable, VtableEntry};
