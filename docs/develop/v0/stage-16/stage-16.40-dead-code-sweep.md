# Stage 16.40 — Clean Up Dead dyn_trait_emit Re-exports + Final Codegen Dead Code Sweep

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.233.1 → v0.234.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 5 "去除兼容思维"

## 1. Executive Summary

Stage 16.40 completes the codegen dead code cleanup by removing the 7 dead
re-exports of `dyn_trait_emit` functions from `codegen/mod.rs`. These
functions were identified as dead exports in the Stage 16.35 analysis but
were not cleaned up until now.

**What was removed**:
- 7 `pub use` re-exports from `codegen/mod.rs` for `dyn_trait_emit::*`
  functions (they're only used by tests, not by production codegen pipeline)

**What was updated**:
- 6 test files updated to use the full module path
  (`landin_compiler::codegen::dyn_trait_emit::*`) instead of the
  convenience re-exports

**Why they were dead**: The production codegen pipeline uses `Emitter`
trait methods (`emit_vtable_global`, `emit_dyn_trait_const`,
`emit_dyn_trait_method_call`) instead of the text-only rendering functions
in `dyn_trait_emit.rs`. The 7 functions are only called by tests that
verify the text rendering.

**Test results**: 7842 tests passing, 0 failures, 0 warnings. No behavior
change.

## 2. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2374/2374 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7842 tests passing, 0 failures, 0 warnings.**

## 3. Version Policy

v0.233.1 → v0.234.0 (minor bump — public API surface change: 7 re-exports
removed from `codegen` module. Tests using the old import path must update
to use `codegen::dyn_trait_emit::*`.)
