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

// Stage 18.128 §13.4 J1-J6: split checker.rs into sub-responsibilities
mod check;
pub mod error;
mod infer;
mod writeback;
// Stage 6.15 (TD-025) sub-modules.
mod predicates;
// Stage 16.68 (Task 17 Phase 3): Associated type projection resolution.
// Stage 17.03: Trait solver (v0.5 Phase 1).
pub mod solver;
mod tables;
// Stage 16.73: Where clause checking.
pub mod where_clause;
// Stage 14.105 (dead code cleanup): `lifetime_elision` module removed.
// It was `#[allow(dead_code)]` since Stage 8.1 and never called.
// Lifetime elision will be re-implemented in v0.2 when real lifetimes are added.
pub mod unify;

// Stage 18.60: Removed deprecated `check_crate` and `check_mir_body_with_hir`
// dead code. The driver now uses `TypeChecker::check_mir_body_with_tables`
// directly (§16-compliant). `check_mir_body` is kept as a convenience wrapper.
pub use checker::{check_mir_body, TypeChecker};
// Stage 6.15: re-export data tables from `tables` sub-module for backward compat.
pub use error::{TypeError, TypeErrorKind};
pub use tables::{FieldTyTable, FnSigTable, TypeckResults};
pub use unify::UnificationTable;
