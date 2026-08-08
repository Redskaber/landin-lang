//! Type checking: type inference + trait resolution.
//!
//! Per 03-type-system.md, the type checker walks MIR bodies, collects
//! type constraints from rvalues/operands, and unifies inference
//! variables to concrete types.
//!
//! ## Canonical entry point (§16-compliant, Stage 3.60+)
//!
//! [`TypeChecker::check_mir_body_with_tables`] — reads zero HIR; receives
//! a pre-computed [`FieldTyTable`] built by the driver. This is the entry
//! point used by `driver::compile`.
//!
//! ## Legacy entry points (deprecated, Stage 3.63)
//!
//! - [`check_crate`] — re-lowers HIR to MIR internally (§16 violation).
//! - [`check_mir_body`] — delegates to `check_mir_body_with_tables(None)`.

pub mod checker;
pub mod error;
// Stage 6.15 (TD-025) sub-modules.
mod predicates;
// Stage 16.68 (Task 17 Phase 3): Associated type projection resolution.
pub mod projection_resolver;
// Stage 17.03: Trait solver (v0.5 Phase 1).
pub mod solver;
mod tables;
// Stage 16.73: Where clause checking.
pub mod where_clause;
// Stage 14.105 (dead code cleanup): `lifetime_elision` module removed.
// It was `#[allow(dead_code)]` since Stage 8.1 and never called.
// Lifetime elision will be re-implemented in v0.2 when real lifetimes are added.
pub mod unify;

// Stage 3.63 (cross-stage naming standardization): `check_crate` and
// `check_mir_body_with_hir` are kept as deprecated legacy entry points
// for backwards compatibility; new code should use
// `TypeChecker::check_mir_body_with_tables` (§16-compliant).
#[allow(deprecated)]
pub use checker::{check_crate, check_mir_body, TypeChecker};
// Stage 6.15: re-export data tables from `tables` sub-module for backward compat.
pub use error::TypeError;
pub use tables::{FieldTyTable, FnSigTable, TypeckResults};
pub use unify::UnificationTable;
