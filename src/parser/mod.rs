//! Parser: Token stream → AST.
//!
//! Based on 02-grammar.md. Hand-written recursive descent + Pratt parser.
//!
//! ## Entry points
//!
//! - [`parse_crate`] — free-function wrapper (Stage 3.63). Convenience
//!   entry that mirrors `lexer::tokenize` / `hir::lower::lower_crate`
//!   / `resolve::resolve_crate` / `codegen::codegen_crate` style.
//! - [`Parser::new`] + [`Parser::parse_crate`] — struct-based entry
//!   for callers that need stateful access (e.g., inspecting errors
//!   incrementally via `Parser::into_errors`).

pub mod error;
// The submodule `parser` has the same name as its parent module; this is a
// clippy::module_inception warning but we keep the structure because the
// `Parser` type lives in `parser::parser` and the parent module just re-exports.
#[allow(clippy::module_inception)]
pub mod parser;

// Stage 6.12 (TD-022) sub-modules of `parser` — declared here so they
// resolve to `src/parser/{expr,generics,items,pat,path,stmt,ty}.rs`
// (sibling to `parser.rs`). Each sub-module adds methods to `impl Parser`
// via its own `impl` block, per §14.4 (refactoring as architecture design).
mod expr;
mod generics;
mod items;
// Stage 18.03: macro_rules! expansion engine.
pub mod macro_expand;
mod pat;
mod path;
mod stmt;
mod ty;

pub use error::{ParseError, ParseErrorKind};
pub use parser::Parser;

use crate::ast::Crate;
use crate::lexer::Token;

/// Stage 3.63 (cross-stage naming standardization): free-function entry
/// point that wraps `Parser::new(...).parse_crate()` + `into_errors()`.
///
/// Mirrors the entry-point style of sibling stages:
/// - `lexer::tokenize(src, interner) -> (Vec<Token>, Vec<LexError>)`
/// - `hir::lower::lower_crate(ast, interner) -> HirCrate`
/// - `resolve::resolve_crate(hir, interner) -> Vec<ResolveError>`
/// - `mir::lower::lower_hir_body_to_mir(body, interner, hir) -> MirBody`
/// - `codegen::codegen_crate(&CompileResult) -> String`
///
/// Callers needing stateful access (incremental error inspection, etc.)
/// can still use `Parser::new(...).parse_crate()` directly.
pub fn parse_crate(tokens: Vec<Token>, interner: &mut lasso::Rodeo) -> (Crate, Vec<ParseError>) {
    let mut p = Parser::new(tokens, interner);
    let krate = p.parse_crate();
    let errors = p.into_errors();
    (krate, errors)
}
