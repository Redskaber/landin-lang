# Stage 16.26 — Capture Closure GEP Debug: Root Cause Found

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.229.3 → v0.229.4
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协"

## 1. Executive Summary

Stage 16.26 debugged the capture closure segfault and found the **root
cause**: LLVMSysEmitter's `emit_function_begin` reuses forward declarations
with mismatched parameter types. The call site declares the function with
`ptr` (OpaquePtr) parameter type, but `emit_function_begin` computes the
parameter type as `Struct({ i32 })` (from `mir_type_to_emit_type_with_layouts`
on Closure type), and reuses the mismatched declaration.

**No behavior change** — capture closures still use inline path. All 7717
tests pass. No-capture closures still use synthesized `call` function.

## 2. Root Cause Analysis

### 2.1 The Problem

Call site (`codegen_terminator.rs`):
```llvm
%v8 = call i32 @closure_call_fn_0(ptr %loc_5, i32 %v7)  ; ptr = OpaquePtr
```

Function definition (`codegen_synthesized_closure_functions`):
```llvm
define i32 @closure_call_fn_0({ i32 } %"%arg0", i32 %"%arg1")  ; { i32 } = Struct
```

Type mismatch: call site passes `ptr`, function expects `{ i32 }`.

### 2.2 Why It Happens

1. **Call site** (`emit_call`): Calls `get_or_declare_function` with arg
   types from `detect_operand_type`. For Closure-typed self arg, the
   terminator codegen passes `OpaquePtr` → function declared with `ptr` param.

2. **Function definition** (`emit_function_begin`): Computes param types
   from `mir.local_decls`. For Closure-typed self (local_idx=1), the
   `is_self_param` check returns `OpaquePtr` → should be `ptr`.

3. **But**: `emit_function_begin` checks `existing` (forward declaration)
   and **always reuses it** (Stage 14.92 Bug X3 fix). The forward
   declaration was created by `get_or_declare_function` at the call site,
   which may have used **different** param types.

4. **The mismatch**: The call site's `detect_operand_type` for the Closure
   arg may return `Struct({ i32 })` instead of `OpaquePtr`, because
   `detect_operand_type` uses `mir_type_to_emit_type_with_layouts` which
   returns `Struct(fields)` for Closure types (not `OpaquePtr`).

### 2.3 The Fix

The fix is to ensure `detect_operand_type` returns `OpaquePtr` for
Closure-typed args at call sites in synthesized closure function calls.
This requires the same scoping as `detect_place_type`: check
`mir.def_id.is_some()` and `local_idx == 1`.

Alternatively, fix `emit_function_begin` to not reuse declarations with
mismatched types (revert Stage 14.92's "always reuse" behavior for
closure functions).

**Status**: Root cause identified, fix deferred to future stage.

## 3. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2249/2249 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7717 tests passing, 0 failures, 0 warnings.**
- **Runtime**: `f(10) = 11` ✅ (no-capture closures)

## 4. Version Policy

v0.229.3 → v0.229.4 (patch bump — root cause analysis, no behavior change.)
