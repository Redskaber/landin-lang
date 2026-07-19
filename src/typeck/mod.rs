//! Type checking: type inference + trait resolution.
//!
//! Per 03-type-system.md, the type checker walks MIR bodies, collects
//! type constraints from rvalues/operands, and unifies inference
//! variables to concrete types.
//!
//! Public entry point: [`check_crate`].

pub mod error;
pub mod unify;

pub use error::TypeError;
pub use unify::UnificationTable;
