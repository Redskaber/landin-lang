//! Type checking: type inference + trait resolution.
//!
//! Per 03-type-system.md, the type checker walks MIR bodies, collects
//! type constraints from rvalues/operands, and unifies inference
//! variables to concrete types.
//!
//! Public entry point: [`check_crate`].

pub mod checker;
pub mod error;
pub mod unify;

pub use checker::{
    check_crate, check_mir_body, FieldTyTable, FnSigTable, TypeChecker, TypeckResults,
};
pub use error::TypeError;
pub use unify::UnificationTable;
