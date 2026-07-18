//! Parser: Token stream → AST.
//!
//! Based on 02-grammar.md. Hand-written recursive descent + Pratt parser.

pub mod error;
// The submodule `parser` has the same name as its parent module; this is a
// clippy::module_inception warning but we keep the structure because the
// `Parser` type lives in `parser::parser` and the parent module just re-exports.
#[allow(clippy::module_inception)]
pub mod parser;

pub use error::ParseError;
pub use parser::Parser;
