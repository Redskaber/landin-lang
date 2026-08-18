# Stage 18.192 — Array Bounds Check (TD-ARRAY-BOUNDS-CHECK)

> **Date**: 2026-08-17
> **Version**: v0.459.0 → v0.460.0
> **Task ID**: stage18.192

## 1. Scope

Per Stage 18.191 task review: insert OOB bounds check for array Index
projection. Previously `arr[5]` on a 3-element array silently returned
garbage. Now it panics via `__landin_panic_bounds_check`.

## 2. Implementation (src/codegen/mir_translation/places.rs)

Added bounds check in `ProjectionElem::Index` codegen path, before GEP:
1. Extract array length N from `TyKind::Array(_, n)` in local_decls
2. Cast index to i64
3. Create i64 length constant in alloca
4. `icmp slt idx, len` → conditional branch
5. If idx >= len: call `__landin_panic_bounds_check(idx, len)` + unreachable
6. If idx < len: continue to GEP + load

Uses unique block names (atomic counter) to avoid collisions when multiple
array accesses exist in the same function.

Per §1.0 原則 4 (报错>静默): OOB must panic, not return garbage.
Per §1.0 原則 6 (通解>特例): one bounds check for all [T; N] arrays.

## 3. Verification

```
arr = [10, 20, 30]
arr[0] = 10  ✅ (in-bounds works)
arr[1] = 20  ✅
arr[2] = 30  ✅
arr[5] → panic: index out of bounds (index=5 len=3)  ✅ (was: garbage 20)
```

## 4. Tests

Promoted Stage 18.182's `array_oob_soft` test to strict `array_oob_panics`
(now asserts exit != 0). All existing tests pass — no regression in
in-bounds array access.

## 5. §3.2 Acceptance

- ✅ cargo check --all-features: 0 errors / 1 warning
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3049 passed
- **Total**: 3707 tests, 0 failures
