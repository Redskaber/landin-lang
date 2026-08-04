# Stage 16.23 — Task 10 Step 3+4: Capture Closure Deref Projection + Scoped Codegen Fixes

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.229.0 → v0.229.1
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协"

## 1. Executive Summary

Stage 16.23 attempted to extend the closure switch to capture closures by
adding Deref projection in `build_synthesized_closure_mir_body`. The
LLVM text emitter IR is correct, but `LLVMSysEmitter` crashes during GEP
codegen for capture closures. Capture closures reverted to inline path.

All 7709 tests pass. No-capture closures still use synthesized `call`
function (verified: `f(10) = 11`).

**Key changes (all permanent, scoped to synthesized functions)**:
1. `build_synthesized_closure_mir_body`: Capture extraction uses
   `Projection(Projection(self, Deref), Field(i, cap_ty))`
2. `detect_place_type`: Returns `OpaquePtr` for Closure-typed self in
   synthesized functions (scoped: `mir.def_id.is_some() && id.0 == 1`)
3. `detect_place_storage_type`: Returns `Struct(fields)` for Closure-typed
   self in synthesized functions (same scope)
4. `codegen_place_load_typed` Field projection: Loads pointer for
   Closure-typed locals in synthesized functions (same scope)
5. `lower_closure_call_to_synthesized`: `Operand::Move` for capture
   closures, `Operand::Copy` for no-capture closures

## 2. What Works

- ✅ No-capture closures: synthesized `call` function (f(10) = 11)
- ✅ All 7709 tests pass
- ✅ Capture closures: inline path (unchanged, all tests pass)

## 3. What Doesn't Work

- 🔧 Capture closures with synthesized `call` function:
  `LLVMSysEmitter` crashes (segfault) during GEP codegen.
  Text emitter IR is correct, but LLVM C API backend crashes.

**Root cause**: The LLVMSysEmitter's GEP handling for `Deref+Field`
projection on Closure-typed self parameter has a bug. The text emitter
produces correct IR (`getelementptr { i32 }, { i32 }* %v3, i32 0, i32 0`),
but the LLVM C API backend crashes when building the same GEP.

**Fix plan**: Debug LLVMSysEmitter's `emit_gep_field` / GEP building
for Closure-typed places. The issue is likely in how the LLVM C API
handles `LLVMBuildGEP2` with the struct type from `Closure` substs.

## 4. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2241/2241 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7709 tests passing, 0 failures, 0 warnings.**
- **Runtime**: `f(10) = 11` ✅

## 5. Version Policy

v0.229.0 → v0.229.1 (patch bump — scoped codegen fixes, capture closures
still use inline path.)
