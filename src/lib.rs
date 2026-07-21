//! Landin Compiler
//!
//! Stage 0 (v0.1.x): Lexer + Parser + AST — COMPLETE
//! Stage 1 (v0.2.x): HIR + Name Resolution — COMPLETE
//! Stage 2 (v0.4.x): MIR + Typeck + Borrowck — COMPLETE
//! Stage 3 (v0.8.x): LLVM Codegen — COMPLETE (soundness-critical limitations closed)
//!   Stage 3.63 (v0.8.7): cross-stage naming standardization per §21 audit
//!     (9 P1 naming fixes + 1 P2 architectural fix; pure refactoring).
//!   Remaining: L1 (PHI optimization), L3 (closures), L5 (traits), L8 (lli) —
//!   deferred to Stage 4+.
//! See `docs/develop/v0/api-naming-standard.md` for the API naming standard.

pub mod ast;
pub mod borrowck;
pub mod codegen;
pub mod diagnostics;
pub mod driver;
pub mod hir;
pub mod lexer;
pub mod mir;
pub mod parser;
pub mod resolve;
pub mod session;
pub mod typeck;

// Stage 3.61: Clear public API surface — re-export the intended entry points.
// Stage 3.63: Naming standardized per docs/develop/v0/api-naming-standard.md.
pub use codegen::codegen_crate;
pub use driver::{compile, CompileErrors, CompileResult};
