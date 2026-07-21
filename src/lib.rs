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
//!   Stage 3.68 (v0.8.12): Visibility checking infrastructure — def_visibility
//!     map + check_visibility hook (stub, ready for Stage 4 nested modules).
//!   Stage 3.69 (v0.8.13): Process v3.16 (§25 阶段末尾深度审查协议) +
//!     Stage 0-3 deep review (GO-WITH-CONDITIONS for Stage 4).
//!   Stage 4.1-4.2 (v0.9.0): Nested module support (recursive build_module_tree
//!     + child ModuleNode) + L1 PHI optimization CLOSED (design decision: rely
//!     on LLVM mem2reg).
//!   Stage 4.3-4.4 (v0.9.1): Visibility enforcement activation (check_visibility
//!     implements pub/private/pub-restricted checks) + L3 closure lowering
//!     (AggregateKind::Closure + TyKind::Closure → empty struct; capture analysis
//!     deferred to Stage 4.5).
//!   Stage 4.5 (v0.9.2): Complete dev-logs for all stages (Stage 1 + Stage 2 +
//!     Stage 4 dev-logs created; Stage 0 + Stage 3 dev-logs updated with
//!     retroactive entries).
//!   Stage 4.6 (v0.9.3): Process v3.17 — §17 测试目录标准化与三阶段文档协议
//!     (开发轮/审查轮/深度审查轮) + standardized tests/ directory structure.
//!   Stage 4.7 (v0.9.4): L3 closure capture analysis — collect_captured_locals
//!     detects external variables referenced in closure body; captures populate
//!     closure struct fields + Aggregate operands.
//!   Stage 4.8 (v0.9.5): tests/ directory restructure — all 13 flat test files
//!     migrated to standardized tests/v0/stage{N}/plan/ per v3.17 §17.1.
//!   Stage 4.9 (v0.9.6): L3 closure call lowering — detect TyKind::Closure in
//!     Call lowering; simplified placeholder (full call deferred to Stage 4.10).
//!   Remaining: L3 full call lowering (Stage 4.10), L5 (traits), L8 (lli).
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
