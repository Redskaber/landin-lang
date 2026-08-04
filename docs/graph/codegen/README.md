# Codegen Graph Directory

> **Date**: 2026-08-04
> **Version**: v0.234.0

## Files

| File | Description |
|------|-------------|
| `architecture.md` | Codegen module architecture (final post-refactoring state) |
| `emitter-trait.md` | Emitter trait hierarchy (39 methods, 3 doc groups) |
| `data-flow.md` | Unified pipeline data flow (MIR → LLVM IR) |
| `backend-comparison.md` | TextEmitter vs LLVMSysEmitter comparison |

## Key Architecture Decisions

1. **Unified pipeline** (Stage 16.37): `run_codegen_pipeline()` — one entry
   point for both backends, one emission order
2. **Text-backend utilities** (Stage 16.35): `emit_type_to_llvm_str`,
   `binop_to_llvm_str` moved from shared `emitter.rs` to `text/mod.rs`
3. **Dead code removed** (Stages 16.35-16.40): `emit_output`,
   `emit_dyn_trait_ptr_type`, `llvm_ptr_str`, `to_context`,
   `predeclare_function`, 7 dead `dyn_trait_emit` re-exports
4. **Trait split deferred** (Stage 16.38): Documentation groups provide
   architectural clarity; physical split blocked by Rust's single-impl-block
   rule
