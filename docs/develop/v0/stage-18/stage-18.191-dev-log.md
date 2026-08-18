# Stage 18.191 — i64 Literal Fix (TD-INT-UINT-VAR partial) + Task Review

> **Date**: 2026-08-17
> **Version**: v0.458.0 → v0.459.0
> **Task ID**: stage18.191

## 1. Task Review

Per §17, assessed remaining TDs and re-planned task graph:
- TD-INT-UINT-VAR: i64 literals > i32 max truncated (pre-existing, immediately fixable)
- TD-ARRAY-BOUNDS-CHECK: OOB detection (immediately fixable, __landin_panic_bounds_check exists)
- Box auto-drop: drop glue (immediately fixable, drop_glue.rs exists)
- Vec<T>, String::push_str: need realloc (blocked)

Selected TD-INT-UINT-VAR as highest priority — affects all i64 literal usage.

## 2. Root Cause

`emit_const` (src/codegen/llvm/arithmetic.rs) always created i32 constants for
`ConstVal::Int(n)` and `ConstVal::Uint(n)`. Values > i32::MAX were silently
truncated by `LLVMConstInt(i32_type, n, ...)`.

The caller (operand.rs) then cast the i32 to the target type — but the value
was already truncated, so the cast couldn't recover it.

## 3. Fix

### 3.1 emit_const (src/codegen/llvm/arithmetic.rs)

Changed to use i64 for values > i32::MAX, i32 otherwise:
```rust
ConstVal::Int(n) => {
    let n_val = *n as u64;
    if n_val <= i32::MAX as u64 {
        LLVMConstInt(LLVMInt32TypeInContext(ctx), n_val, 1)  // i32 for small
    } else {
        LLVMConstInt(LLVMInt64TypeInContext(ctx), n_val, 1)  // i64 for large
    }
}
```

Per §1.0 原則 9 (正确>妥协): use the minimum type that preserves the value.
Per §1.0 原則 6 (通解>特例): one rule for all integer constants.

### 3.2 operand.rs src_ty (src/codegen/operand.rs)

Updated `src_ty` to match the actual LLVM type created by `emit_const`:
- Small values (≤ i32::MAX): src_ty = I32 (no cast needed for i32 targets)
- Large values (> i32::MAX): src_ty = I64 (cast/promote to target)

### 3.3 operand.rs target_ty promotion

Added value-based promotion: if the constant value > i32::MAX and target_ty
is I32, promote target_ty to I64. This handles cases where typeck resolves
the constant type to i32 (default) but the value needs i64.

### 3.4 Test fix (src/codegen/llvm/tests.rs)

`test_simple_module_builds_and_emits` now casts the i64 const to i32 before
returning (matching the function's return type).

## 4. Verification

```
let v: i64 = 3000000000 → 3000000000  ✅ (was: -1294967296)
let v: i64 = 1000000000000 → 1000000000000  ✅
const MAX: i32 = 100 → store i32 100  ✅ (preserved, no trunc)
while i < 5 { ... } → 0 1 2 3 4  ✅ (no type mismatch)
```

## 5. §3.2 Acceptance

- ✅ cargo check --all-features: 0 errors / 1 warning
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3049 passed
- **Total**: 3707 tests, 0 failures
