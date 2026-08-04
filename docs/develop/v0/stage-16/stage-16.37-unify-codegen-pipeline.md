# Stage 16.37 — Unify Codegen Pipeline: Shared Driver for All Backends

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.232.1 → v0.233.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例"

## 1. Executive Summary

Stage 16.37 unifies the codegen pipeline by extracting a single shared
`run_codegen_pipeline` function that both `codegen_crate` (text backend)
and `codegen_crate_to_module` (LLVM backend) delegate to. This eliminates
the duplicate entry-point logic and the inverted emission order between
text and LLVM backends.

**Key achievement**: One pipeline, one emission order, zero duplication.

**Before** (two divergent entry points):
```
codegen_crate (text):              codegen_crate_to_module (LLVM):
1. emit_header + declares          1. emit_header + declares
2. codegen_from_mir                2. set_fn_sigs (LLVM-only)
3. codegen_synthesized_closures    3. emit_vtables ← globals FIRST
4. emit_vtables ← globals AFTER    4. emit_dyn_trait_ptrs
5. emit_dyn_trait_ptrs             5. emit_drop_glue
6. emit_drop_glue                  6. codegen_from_mir
7. output_with_globals()           7. codegen_synthesized_closures
                                   8. return emitter
```

**After** (unified pipeline):
```
run_codegen_pipeline (shared):
1. emit_header + declares
2. emit_vtables
3. emit_dyn_trait_ptrs
4. emit_drop_glue
5. codegen_from_mir
6. codegen_synthesized_closures

codegen_crate:                    codegen_crate_to_module:
  TextEmitter::new()                LLVMSysEmitter::new()
  set_fn_sigs (if LLVM)             set_fn_sigs (if LLVM)
  run_codegen_pipeline(result, &mut emitter)
  output_with_globals()             return emitter
```

**Test results**: 7814 tests passing (244 lib + 2346 integration + 5224
conformance), 0 failures, 0 warnings. Runtime verified.

## 2. The Problem

Before Stage 16.37, the two codegen entry points had divergent logic:
- **Different emission orders**: Text emitted globals AFTER function bodies;
  LLVM emitted globals BEFORE function bodies (for forward-reference resolution).
- **Duplicated code**: Both entry points had the same 6-step pipeline logic,
  copy-pasted with different orders.
- **Maintenance risk**: Changes to one entry point could be forgotten in the
  other, causing text/LLVM IR divergence.

## 3. The 通解 Fix

Extracted `run_codegen_pipeline(result, &mut dyn Emitter)` — a single
function that contains the unified emission order:

1. Module header + panic declarations
2. Vtable globals (BEFORE function bodies — LLVM needs forward refs)
3. Dyn trait fat-pointer globals
4. Drop glue functions
5. Main MIR function bodies (codegen_from_mir)
6. Synthesized closure function bodies

The text backend buffers globals separately (in `globals: Vec<String>`) and
appends them at output time (via `output_with_globals`), so the "globals
first" order works for both backends — LLVM IR text allows globals before
function definitions.

The LLVM-specific setup (`set_fn_sigs`) is done in the entry point BEFORE
calling `run_codegen_pipeline`, so the pipeline itself remains
backend-agnostic.

Per §1.0 原則 6 "通用 > 特例": one pipeline for all backends.
Per §23: clear single entry point for the codegen pipeline.

## 4. API

```rust
/// Unified codegen pipeline — shared by both text and LLVM backends.
pub fn run_codegen_pipeline(
    result: &crate::driver::CompileResult,
    emitter: &mut dyn Emitter,
)
```

Both entry points are now thin wrappers:
- `codegen_crate`: `TextEmitter::new()` → `run_codegen_pipeline` → `output_with_globals()`
- `codegen_crate_to_module`: `LLVMSysEmitter::new()` → `set_fn_sigs` → `run_codegen_pipeline` → return emitter

## 5. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2346/2346 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7814 tests passing, 0 failures, 0 warnings.**
- **Runtime**: f(10)=11 ✅, f()()()=42 ✅, mut_cap=3 ✅

## 6. Version Policy

v0.232.1 → v0.233.0 (minor bump — new public API `run_codegen_pipeline`.
This is a structural improvement that changes the codegen module's public
interface, warranting a minor version bump.)
