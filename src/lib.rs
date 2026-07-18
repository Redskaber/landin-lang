//! Landin Compiler (Stage 0)
//!
//! 用 Rust 实现的 Landin 语言编译器。
//! 本阶段实现：Lexer + Parser + AST。

pub mod ast;
pub mod diagnostics;
pub mod lexer;
pub mod parser;
pub mod session;
