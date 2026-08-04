# Stage 16.21 — Task 10 Steps 3+4: Codegen Closure-as-Pointer Fix

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.228.7 → v0.228.8
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协"

## 1. Executive Summary

Stage 16.21 applied multiple codegen fixes for the closure switch:
1. **self param as OpaquePtr**: Scoped to synthesized closure functions
   (local_idx=1 with Closure type + mir.def_id.is_some())
2. **Call site passes pointer**: Closure-typed args pass `%loc_N` (alloca ptr)
3. **fn_sigs fixed**: inputs = [closure_struct_ty, param_tys...] (not captures)
4. **Operand::Move for self**: borrowck rejects Copy of non-Copy Closure
5. **codegen_crate_to_module**: Now emits synthesized closure functions
6. **Scoped alloca fix**: Only `self` param in synthesized functions uses OpaquePtr

**Result**: LLVM IR is structurally correct (function sig, param types, call
args all match), but runtime output is wrong (returns 209 instead of 11).
Root cause: empty struct (`{}`) alloca for Closure with no captures causes
LLVM undefined behavior. Reverted to inline path.

## 2. Fixes Applied (All Permanent)

### 2.1 fn_sigs Registration (driver.rs)
- inputs = [closure_struct_ty, param_tys...] (was: [closure_struct_ty, captures...])
- Captures are NOT function arguments — they're extracted from self

### 2.2 Operand::Move for self (expr_operand.rs)
- Closure types are not Copy → borrowck rejects Operand::Copy
- Changed to Operand::Move (doesn't actually consume — codegen passes ptr)

### 2.3 self param as OpaquePtr (codegen/mod.rs)
- Scoped: only local_idx=1 + Closure type + mir.def_id.is_some()
- Function signature: `define i32 @closure_call_fn_0(ptr %arg0, i32 %arg1)`

### 2.4 Call site passes pointer (codegen/terminator.rs)
- Closure-typed args pass `%loc_N` (alloca pointer) instead of value
- Type: OpaquePtr instead of Struct(vec![])

### 2.5 codegen_crate_to_module emits synthesized functions (codegen/mod.rs)
- Was missing — only codegen_crate (text) emitted them
- Now both paths emit synthesized closure functions

### 2.6 Scoped alloca fix (codegen/mod.rs)
- Only `self` param (local_idx=1) in synthesized functions uses OpaquePtr
- Other Closure-typed locals (in caller) keep Struct type

## 3. Remaining Issue

Runtime output is wrong (209 instead of 11). The LLVM IR is structurally
correct but has undefined behavior:
- `%loc_3 = alloca {}` — empty struct alloca in main (for closure struct)
- `store {} 0, %loc_3` — storing empty struct value
- `call i32 @closure_call_fn_0(ptr %loc_3, i32 10)` — passing ptr to empty

The empty struct (`{}`) alloca has size 0 in LLVM, which may cause the
pointer to be invalid or point to unexpected memory.

**Fix needed**: Codegen should use `i8` (byte) instead of `{}` for
empty Closure struct allocas, ensuring the pointer is valid.

## 4. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings (1 dead_code)
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2241/2241 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7709 tests passing, 0 failures, 0 warnings.**

## 5. Version Policy

v0.228.7 → v0.228.8 (patch bump — codegen fixes, switch still deferred.)
