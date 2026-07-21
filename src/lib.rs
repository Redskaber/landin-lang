//! Landin Compiler
//!
//! Stage 0 (v0.1.x): Lexer + Parser + AST — COMPLETE
//! Stage 1 (v0.2.x): HIR + Name Resolution — COMPLETE (Stage 3.64: `use` decl resolution;
//!   Stage 3.65: `unsafe impl/trait` AST fields + `Res::SelfTy` trait/impl discrimination)
//! Stage 2 (v0.4.x): MIR + Typeck + Borrowck — COMPLETE (Stage 3.65: `lower_body` aliases)
//! Stage 3 (v0.8.x): LLVM Codegen — COMPLETE (soundness-critical limitations closed)
//!   Stage 3.63 (v0.8.7): cross-stage naming standardization per §21 audit
//!     (9 P1 naming fixes + 1 P2 architectural fix; pure refactoring).
//!   Stage 3.64 (v0.8.8): P2 ergonomics fixes + use declaration resolution
//!     (6 Error trait impls + Emitter re-export + emit_output rename +
//!      basic use resolution: leaf/glob/path-prefix/alias).
//!   Stage 3.65 (v0.8.9): P2 architectural fixes
//!     (unsafe impl/trait AST+HIR+parser + Res::SelfTy HirSelfKind discrimination +
//!      lower_body aliases + mir_type_to_emit_type documentation).
//!   Stage 3.66 (v0.8.10): Lvalue → Place rename (167+ refs, aligns with design
//!     doc 06-mir.md §4 + borrowck vocabulary) + resolver owner context threading
//!     for accurate HirSelfKind (Trait vs Impl).
//!   Stage 3.67 (v0.8.11): P2 cleanup — body owner context threading (body-level
//!     HirSelfKind accurate) + &mut Rodeo → &Rodeo in resolve_crate (lexer now
//!     interns keywords) + Span::DUMMY placeholders fixed (11 occurrences in
//!     parser.rs → keyword spans).
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
// Stage 3.64: Re-export codegen Emitter trait + impls for pluggability
// (allows third-party LLVM-IR backends to implement `Emitter` and call
// `codegen_from_mir` directly).
pub use codegen::{codegen_crate, EmitType, EmitValue, Emitter, TextEmitter};
pub use driver::{compile, CompileErrors, CompileResult};
