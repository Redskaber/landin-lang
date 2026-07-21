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
pub mod unify;

// Stage 3.63 (cross-stage naming standardization): `check_crate` and
// `check_mir_body_with_hir` are kept as deprecated legacy entry points
// for backwards compatibility; new code should use
// `TypeChecker::check_mir_body_with_tables` (§16-compliant).
#[allow(deprecated)]
pub use checker::{
    check_crate, check_mir_body, FieldTyTable, FnSigTable, TypeChecker, TypeckResults,
};
pub use error::TypeError;
pub use unify::UnificationTable;
