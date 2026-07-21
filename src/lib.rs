//! Landin Compiler
//!
//! Stage 0 (v0.1.x): Lexer + Parser + AST — COMPLETE
//! Stage 1 (v0.2.x): HIR + Name Resolution — COMPLETE
//! Stage 2 (v0.4.x): MIR + Typeck + Borrowck — COMPLETE
//! Stage 3 (v0.5.x): LLVM codegen — IN PROGRESS

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
pub use codegen::codegen_crate;
pub use driver::{compile, CompileErrors, CompileResult};
