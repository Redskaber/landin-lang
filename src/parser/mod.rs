//! Parser: Token stream → AST.
//!
//! Based on 02-grammar.md. Hand-written recursive descent + Pratt parser.

pub mod error;
pub mod parser;

pub use error::ParseError;
pub use parser::Parser;
