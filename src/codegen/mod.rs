//! LLVM IR codegen: MIR → LLVM IR via Emitter trait.
//!
//! ## Status
//!
//! Stage 3 (v0.8.x) is COMPLETE. `codegen_crate` is §16-compliant —
//! it takes a `&CompileResult` (pre-built MIR + pre-computed metadata)
//! and makes zero upstream function calls (no `crate::mir::lower`,
//! no `crate::typeck`, no `crate::driver` beyond type-only references).
//!
//! Stage 3.46 (v0.8.6): full integer type support (i8/i16/i32/i64/i128).
//! Stage 3.63 (cross-stage naming standardization): `fat_ptr_type` →
//! `emit_fat_ptr_type` for prefix consistency with the
//! `mir_type_to_emit_type` / `emit_type_to_llvm_str` translation ladder.
//!
//! ## Open limitations (deferred to Stage 4+)
//!
//! All soundness-critical limitations are CLOSED. The 5 remaining open
//! limitations are soundness-non-critical and explicitly deferred:
//!
//! | ID | Description | Target |
//! |----|-------------|--------|
//! | L1 | PHI node optimization — **CLOSED in Stage 4.2** (design decision: rely on LLVM `mem2reg` rather than emitting PHI directly; documented below) | ✅ |
//! | L3 | Closure codegen — **IN PROGRESS (Stage 4.9)**: closure type lowering + capture analysis + closure call detection. Full call lowering (extract captures + invoke body) deferred to Stage 4.10. | Stage 4.10 |
//! | L5 | Trait dispatch (vtable generation, dyn fat pointers) | Stage 5 |
//! | L8 | `lli` execution verification (env constraint — no `lli` in test sandbox) | Stage 4 |
//! | L-COPY-ADT | Proper Copy trait (current borrowck pragmatically treats Adt as Copy) | Stage 5 |
//!
//! ## L1 PHI optimization — design decision (Stage 4.2)
//!
//! **Decision**: Landin codegen emits `alloca` + `load` + `store` for all
//! locals, and relies on LLVM's `mem2reg` optimization pass to produce SSA
//! form with PHI nodes. This is the **standard approach** used by Clang,
//! rustc, and most LLVM frontends.
//!
//! **Rationale**:
//! 1. `mem2reg` is a well-tested LLVM pass that produces optimal SSA form
//! 2. Implementing PHI emission manually would duplicate `mem2reg` logic
//!    and risk correctness bugs
//! 3. The current `alloca`-based IR is **correct** — it produces valid
//!    LLVM IR that any LLVM toolchain can optimize
//! 4. The IR quality concern is **non-blocking** — `opt -mem2reg` or
//!    `lli` (which runs default passes) produces optimal code
//!
//! **What was considered and rejected**: Emitting PHI nodes directly in
//! `codegen_function` by tracking SSA values per basic block. This would
//! require:
//! - A per-block value mapping (local → SSA value)
//! - PHI node insertion at block joins
//! - Dominance frontier computation
//! - Handling of partially-defined variables
//!
//! This is essentially reimplementing `mem2reg` in Rust — high effort,
//! high risk, low benefit over just running `opt -mem2reg`.
//!
//! **Conclusion**: L1 is **CLOSED** as a design decision. The `alloca`-
//! based IR is the intended design, not a limitation to be fixed.
//!
//! ## Architectural debt (tracked, not blocking)
//!
//! - **Emitter trait split** (Stage 16.76 MUV-1, COMPLETE): 39 methods,
//!   2 implementations (TextEmitter + LLVMSysEmitter). Split into 6
//!   sub-traits (ModuleEmitter, FunctionEmitter, ArithmeticEmitter,
//!   MemoryEmitter, AggregateEmitter, LocalStateEmitter) per §13.4 J2
//!   single responsibility. `Emitter` is now a super-trait with a blanket
//!   impl — `&mut dyn Emitter` callers (20 sites) keep working; external
//!   implementers must implement 6 sub-traits individually.

use crate::driver::CompileResult;
use crate::mir::body::*; // Re-exported for sub-modules via `super::*`
use lasso::Rodeo; // Re-exported for sub-modules via `super::*`

pub mod emitter;
// Stage 17.01-17.02: CodegenError error system.
pub mod error;
// Stage 18.88: Cross-compilation target triple support.
pub mod target;
pub use emitter::{
    emit_fat_ptr_type, mir_type_to_emit_type, AggregateEmitter, ArithmeticEmitter, EmitType,
    EmitValue, Emitter, FunctionEmitter, LocalStateEmitter, MemoryEmitter, ModuleEmitter,
};
pub use error::{CodegenError, CodegenResult};
pub use target::TargetTriple;

pub mod text;
pub use text::TextEmitter;

// Stage 13.5 MUV-2: LLVMSysEmitter — LLVM C-API emitter via llvm-sys.
// Only available behind the `llvm-backend` feature.
#[cfg(feature = "llvm-backend")]
pub mod llvm;
#[cfg(feature = "llvm-backend")]
pub use llvm::LLVMSysEmitter;

// Stage 13.28: Codegen sub-modules (extracted from mod.rs for better
// organization and maintainability).
mod operand;
mod rvalue;
mod statement;
mod terminator;
mod trait_dispatch;

// Stage 18.157: Shared C runtime wrapper (used by landin-stage0 + landinc).
pub mod runtime;

// Stage 16.76 MUV-2: Pipeline orchestrator + per-function codegen + drop glue.
mod drop_glue;
mod function;
mod pipeline;

// Re-export functions from sub-modules for use within codegen.
// Stage 15.65: `codegen_dyn_trait_call` (legacy) removed; use `_direct` variant.
pub use operand::codegen_dyn_trait_call_direct;
pub(crate) use operand::codegen_operand;
pub(crate) use rvalue::codegen_rvalue;

// Stage 16.76 MUV-2: Re-export pipeline + function helpers.
pub(crate) use pipeline::run_codegen_pipeline;

// mir_translation helpers — pub so lib.rs can re-export; pub(crate) for
// sub-module access via super::*
pub(crate) mod mir_translation;
pub use mir_translation::{
    mir_type_to_emit_type_with_layouts, mir_type_to_emit_type_with_layouts_and_mono,
    stdlib_type_kind_to_emit_type,
};

// Stage 13.1 (TD-028): dyn Trait LLVM IR text emission relocated from
// `mir::dyn_trait` per §16 interface isolation fix. These 7 emit_*
// functions are pure "MIR data → LLVM IR text" converters and belong
// in codegen, not MIR.
pub mod dyn_trait_emit;
// Stage 16.40: Removed dead re-exports of dyn_trait_emit functions.
// These 7 functions are only used by tests (not by production codegen
// pipeline, which uses Emitter trait methods). Tests should use the
// full module path: `landin_compiler::codegen::dyn_trait_emit::*`.
// Per §1.0 原則 5 "去除兼容思维": dead re-exports removed.

// Stage 6.7: emit_vtables and emit_dyn_trait_ptrs re-exported from trait_dispatch.
pub use trait_dispatch::{
    build_dynptr_global_specs, build_trait_dispatch_emission_plan,
    build_trait_dispatch_emission_summary, build_vtable_global_specs, emit_dyn_trait_ptrs,
    emit_dynptr_global_text, emit_dynptrs_from_resolver, emit_trait_dispatch_globals_from_plan,
    emit_trait_dispatch_globals_text_batch, emit_trait_dispatch_globals_text_batch_from_resolver,
    emit_vtable_global_from_emission, emit_vtable_global_text, emit_vtable_globals_batch,
    emit_vtables, emit_vtables_and_dynptrs_from_resolver, emit_vtables_from_resolver,
    CodegenTraitDispatchEmissionPlan, CodegenTraitDispatchEmissionSummary, StdlibDynptrGlobalSpec,
    StdlibVtableGlobalSpec,
};

/// Stage 3.56 (Phase A §16 refactoring): Generate LLVM IR from a
/// `CompileResult` — codegen is now a **pure MIR consumer**.
///
/// Was (Stage 3.1-3.55): codegen re-lowered HIR to MIR + re-ran typeck
/// inside codegen, violating section 16. Also silently skipped borrowck
/// and dropped type errors.
///
/// Now: codegen reads pre-built MIR + pre-computed metadata. Zero
/// calls to upstream stage functions.
///
/// Stage 16.37: Both `codegen_crate` and `codegen_crate_to_module` now
/// delegate to the shared `run_codegen_pipeline` function, which contains
/// the unified emission order. This eliminates the duplicate entry-point
/// logic and the inverted emission order between text and LLVM backends.
///
/// Stage 16.76 MUV-2: `run_codegen_pipeline` moved to `pipeline.rs`;
/// `codegen_function` / `codegen_from_mir` / `codegen_synthesized_closure_functions`
/// moved to `function.rs`; `emit_drop_glue_functions` moved to `drop_glue.rs`.
/// This mod.rs now contains only the two public entry points + module
/// declarations + re-exports.
/// Stage 18.151 (TD-CODEGEN-RESULT): `codegen_crate` now returns
/// `CodegenResult<String>` to propagate codegen errors from the pipeline.
///
/// Per §2 原则 9 (正确>妥协): full Result propagation, no panic stubs.
/// Per §12 (最优>最小): root-cause fix.
pub fn codegen_crate(result: &CompileResult) -> CodegenResult<String> {
    codegen_crate_with_target(result, TargetTriple::default())
}

/// Stage 18.89: Generate LLVM IR text with a specific target triple.
/// Stage 18.151 (TD-CODEGEN-RESULT): Returns `CodegenResult<String>`.
pub fn codegen_crate_with_target(
    result: &CompileResult,
    target: TargetTriple,
) -> CodegenResult<String> {
    let mut emitter = TextEmitter::with_target(target);
    run_codegen_pipeline(result, &mut emitter)?;
    Ok(emitter.output_with_globals())
}

/// Stage 13.5 MUV-2: Generate LLVM IR via the LLVM C API (`llvm-sys`).
///
/// Mirrors `codegen_crate` but uses `LLVMSysEmitter` instead of
/// `TextEmitter`. The returned `LLVMModuleRef` is owned by the
/// `LLVMSysEmitter` instance returned alongside it (so callers can
/// drop them together). Use `LLVMSysEmitter::to_module()` to access
/// the module and `LLVMSysEmitter::to_object_file()` to emit an
/// object file.
///
/// Per §16: same MIR-only consumer contract as `codegen_crate` —
/// zero upstream calls to `crate::mir::lower` / `crate::typeck`.
///
/// Stage 16.37: Delegates to the shared `run_codegen_pipeline` function.
/// The LLVM-specific pre-pipeline setup (`set_fn_sigs`) is done via the
/// concrete `LLVMSysEmitter` type (not via trait) — the pipeline itself
/// remains backend-agnostic via `&mut dyn Emitter`.
///
/// Stage 16.76 MUV-2: `build_fn_sigs_map` moved to `llvm/function_sigs.rs`.
/// Stage 18.151 (TD-CODEGEN-RESULT): Returns `CodegenResult<LLVMSysEmitter>`.
#[cfg(feature = "llvm-backend")]
pub fn codegen_crate_to_module(result: &CompileResult) -> CodegenResult<LLVMSysEmitter> {
    codegen_crate_to_module_with_target(result, TargetTriple::default())
}

/// Stage 18.89: Generate LLVM module with a specific target triple.
/// Stage 18.151 (TD-CODEGEN-RESULT): Returns `CodegenResult<LLVMSysEmitter>`.
#[cfg(feature = "llvm-backend")]
pub fn codegen_crate_to_module_with_target(
    result: &CompileResult,
    target: TargetTriple,
) -> CodegenResult<LLVMSysEmitter> {
    let mut emitter = LLVMSysEmitter::with_target(target);
    // Stage 14.91 (Bug X3 fix): Populate fn_sigs BEFORE vtable emission.
    let fn_sigs_map = crate::codegen::llvm::function_sigs::build_fn_sigs_map(
        &result.fn_name_by_def_id,
        &result.fn_sigs,
    );
    emitter.set_fn_sigs(fn_sigs_map);
    run_codegen_pipeline(result, &mut emitter)?;
    Ok(emitter)
}
