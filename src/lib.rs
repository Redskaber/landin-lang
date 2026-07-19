//! Landin Compiler
//!
//! Stage 0 (v0.1.x): Lexer + Parser + AST — COMPLETE at v0.1.4
//! Stage 1 (v0.2.x): HIR + Name Resolution — IN PROGRESS
//!   1.1: HIR data structures (this version)
//!   1.2: AST → HIR lowering (next)
//!   1.3: Module-level name resolution
//!   1.4: Scope-based name resolution

pub mod ast;
pub mod diagnostics;
pub mod hir;
pub mod lexer;
pub mod mir;
pub mod parser;
pub mod resolve;
pub mod session;
pub mod typeck;
