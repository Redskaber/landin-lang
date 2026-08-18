# Stage 18.195 — Vec<T> MVP (TD-VEC-MVP partial)

> **Date**: 2026-08-17
> **Version**: v0.461.0 → v0.462.0
> **Task ID**: stage18.195

## 1. Scope

Per Stage 18.191 task review: implement Vec<T> MVP — prelude struct + Vec::new + Vec::len.
Vec::push is a stub (deferred — needs complex MIR control flow for alloc/realloc).

## 2. Implementation

### 2.1 Vec struct (src/stdlib/prelude.rs)

Added `struct Vec<T> { ptr: *mut T, len: i64, cap: i64 }` to prelude.

Per §1.0 原則 6 (通解>特例): one Vec type for all T (generic, not per-type).
Per §2 原則 9 (正确>妥协): MVP uses ptr/len/cap layout (simplified from Rust's
Vec<T> { buf: RawVec<T>, len }). Simplification acceptable for v0.1.

### 2.2 Vec::new() MIR intrinsic (src/mir/lower/expr_variants.rs)

Added `lower_vec_new_intrinsic` — creates Vec { ptr: null, len: 0, cap: 0 }.
No allocation on creation (lazy alloc on first push).

### 2.3 Vec::len() MIR intrinsic

Added interception in `lower_method_call_expr` — extracts field 1 (len) from
Vec struct, same pattern as str::len and String::len.

### 2.4 Vec::push() stub

Vec::push compiles but is a no-op stub. Full implementation requires complex
MIR control flow (check len == cap, alloc/realloc, store, increment len).
Deferred to Stage 18.196+ (TD-VEC-PUSH-NOTIMPLEMENTED).

### 2.5 Test fix (tests/v0/stage18/plan/stage18_98_103_monomorphization_tests.rs)

Renamed test struct `Vec<T>` to `MyVec<T>` to avoid collision with prelude Vec.

## 3. Verification

```
Vec::new().len() = 0         ✅
Vec::new().len = 0, .cap = 0  ✅
Vec::new() in prelude        ✅
Multiple Vec::new() calls    ✅
```

## 4. Tests

4 tests (all positive). All 4 pass.

## 5. §3.2 Acceptance

- ✅ cargo fmt --check: exit 0
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3057 passed (was 3053, +4 new)
- **Total**: 3715 tests, 0 failures

## 6. Tech Debt

| ID | Status |
|----|--------|
| TD-VEC-MVP | 🟡 Partial — Vec::new + Vec::len done; Vec::push deferred |
| TD-VEC-PUSH-NOTIMPLEMENTED | 🟡 New — Vec::push is a no-op stub |
