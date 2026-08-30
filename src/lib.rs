//! # Landin Compiler
//!
//! A work-in-progress systems programming language inspired by Rust, using
//! LLVM 22 (llvm-sys 221) for code generation. The compiler is written in
//! Rust (~50,000 LOC) and targets x86_64 + AArch64 Linux.
//!
//! ## Crate Layout
//!
//! | Module      | Responsibility                                     |
//! |-------------|----------------------------------------------------|
//! | `lexer`     | Source text → token stream                         |
//! | `parser`    | Token stream → AST + macro expansion               |
//! | `hir`       | AST → HIR (name resolution target)                 |
//! | `resolve`   | HIR path resolution (`use` decls, scopes)           |
//! | `mir`       | HIR → MIR (control-flow graph + types)             |
//! | `typeck`    | MIR type checking + unification                     |
//! | `borrowck`  | Ownership + borrow + NLL liveness                  |
//! | `codegen`   | MIR → LLVM IR text + LLVM sys module (opt)         |
//! | `driver`    | Pipeline orchestration (lex → parse → ... → codegen)|
//! | `stdlib`    | Core/alloc/std type registry + vtable layout       |
//! | `traits`    | TraitResolver + coherence + vtable dispatch        |
//! | `diagnostics` | Error rendering (color, source context)         |
//! | `session`   | SourceFile + SourceMap + Span                      |
//! | `cargo`     | Mini-cargo manifest + build orchestration           |
//!
//! ## Public Entry Points
//!
//! - [`compile`](driver::compile) — single-file compile (lex → codegen)
//! - [`compile_project`](driver::compile_project) — multi-file project compile
//! - [`build_project`](cargo::build_project) — mini-cargo build orchestration
//!
//! ## Versioning
//!
//! - **Current**: v0.493.0 (Stage 18.312)
//! - **Status**: v0.4 stable. 4203 tests, 0 failures.
//! - **History**: see `RELEASE_NOTES.md` + `docs/worklog.md`
//!
//! ## Design Documents
//!
//! - `docs/lang-design/` — language specification (00-18)
//! - `docs/graph/` — pipeline + data-flow diagrams
//! - `docs/stage-committee-process.md` — development process SOP
//! - `docs/develop/v0/tech-debt-register.md` — tech debt tracking
//!
//! Per §1.0 原則 3 (显式 > 隐式): stage-by-stage history lives in
//! `RELEASE_NOTES.md` + `docs/worklog.md`, NOT in this crate-level doc.
//! Per §1.0 原則 5 (去除兼容思维): historical stage log removed (was 405 lines
//! of Stage 0-5 sub-stage descriptions; superseded by RELEASE_NOTES.md).

pub mod ast;
pub mod borrowck;
pub mod cargo;
pub mod codegen;
pub mod diagnostics;
pub mod driver;
pub mod hir;
pub mod lexer;
pub mod mir;
pub mod parser;
pub mod resolve;
pub mod session;
pub mod stdlib;
pub mod traits;
pub mod typeck;

// Stage 3.61: Clear public API surface — re-export the intended entry points.
// Stage 3.63: Naming standardized per docs/develop/v0/api-naming-standard.md.
// Stage 3.64: Re-export codegen Emitter trait + impls for pluggability
// (allows third-party LLVM-IR backends to implement `Emitter` and call
// `codegen_from_mir` directly).
pub use cargo::{build_project, BuildConfig, BuildResult, ProjectManifest};
pub use codegen::{
    build_dynptr_global_specs, build_trait_dispatch_emission_plan,
    build_trait_dispatch_emission_summary, build_vtable_global_specs, codegen_crate,
    codegen_dyn_trait_call_direct, emit_dyn_trait_ptrs, emit_dynptr_global_text,
    emit_dynptrs_from_resolver, emit_trait_dispatch_globals_from_plan,
    emit_trait_dispatch_globals_text_batch, emit_trait_dispatch_globals_text_batch_from_resolver,
    emit_vtable_global_from_emission, emit_vtable_global_text, emit_vtable_globals_batch,
    emit_vtables, emit_vtables_and_dynptrs_from_resolver, emit_vtables_from_resolver,
    stdlib_type_kind_to_emit_type, CodegenTraitDispatchEmissionPlan,
    CodegenTraitDispatchEmissionSummary, EmitType, EmitValue, Emitter, StdlibDynptrGlobalSpec,
    StdlibVtableGlobalSpec, TextEmitter,
};
pub use driver::{
    compile, compile_no_opt, compile_project, compile_project_from_manifest, compile_project_opt,
    CompileErrors, CompileResult, ModuleLoadError, ModuleLoader,
};
// Stage 18.95: TraitError moved from driver.rs to traits/error.rs.
pub use stdlib::{
    default_prelude, find_stdlib_trait_method, integer_bit_width, is_float_type, is_primitive_type,
    is_signed_integer, is_stdlib_marker_trait, is_stdlib_trait, is_stdlib_trait_method,
    is_unsigned_integer, is_zero_sized_type, register_stdlib, resolve_stdlib_type,
    stdlib_all_traits, stdlib_arithmetic_traits, stdlib_core_traits, stdlib_data_global_name,
    stdlib_dynptr_global_name, stdlib_impl_method_symbol, stdlib_io_traits, stdlib_marker_traits,
    stdlib_pointer_width_bytes, stdlib_trait_count, stdlib_trait_method_count,
    stdlib_trait_method_index, stdlib_trait_method_is_unsafe, stdlib_trait_method_param_count,
    stdlib_trait_method_param_kinds, stdlib_trait_method_return_kind,
    stdlib_trait_method_self_kind, stdlib_trait_methods, stdlib_trait_methods_by_is_unsafe,
    stdlib_trait_methods_by_param_count, stdlib_trait_methods_by_return_kind,
    stdlib_trait_methods_by_self_kind, stdlib_traits_with_method, stdlib_traits_with_vtable,
    stdlib_unary_traits, stdlib_vtable_byte_size, stdlib_vtable_emission,
    stdlib_vtable_emission_summary, stdlib_vtable_emissions_for_traits, stdlib_vtable_global_name,
    stdlib_vtable_layout, stdlib_vtable_method_offset, stdlib_vtable_method_symbols,
    stdlib_vtable_plan, stdlib_vtable_plan_entry_count, stdlib_vtable_plan_is_complete,
    stdlib_vtable_plan_missing_methods, stdlib_vtable_slot_count, type_alignment_bytes,
    type_description, type_size_bytes, StdlibFacade, StdlibLayer, StdlibPointerWidth,
    StdlibPrelude, StdlibSelfKind, StdlibTraitMethod, StdlibTypeKind, StdlibVtableEmission,
    StdlibVtableEmissionSummary, StdlibVtablePlan, StdlibVtablePlanEntry, StdlibVtableSlot,
};
pub use traits::TraitError;
pub use traits::{
    extract_impl_self_ty_name, is_primitive_copy_kind, CoherenceError, ImplValidationReport,
    IncompleteImpl, TraitResolver, BUILTIN_DEF_ID_BASE, BUILTIN_PRIMITIVE_COPY_KINDS,
    BUILTIN_TRAIT_NAMES,
};
