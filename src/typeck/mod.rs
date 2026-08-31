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
//! ## Convenience wrapper
//!
//! - [`check_mir_body`] — free function that constructs a `TypeChecker`
//!   with default state and delegates to `check_mir_body_with_tables(None)`.
//!   Used by tests + simple callers that don't need pre-computed tables.
//!
//! ## Stage 18.60 cleanup
//!
//! The deprecated `check_crate` and `check_mir_body_with_hir` free functions
//! were REMOVED in Stage 18.60 — they re-lowered HIR to MIR internally
//! (§16 violation). The driver now uses `check_mir_body_with_tables`
//! directly. See `docs/worklog.md` Stage 18.60 for details.

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
pub mod unify;
pub mod where_clause;

pub use checker::{check_mir_body, TypeChecker};
// Stage 6.15: re-export data tables from `tables` sub-module for backward compat.
pub use error::{TypeError, TypeErrorKind};
pub use tables::{FieldTyTable, FnSigTable, TypeckResults};
pub use unify::UnificationTable;
