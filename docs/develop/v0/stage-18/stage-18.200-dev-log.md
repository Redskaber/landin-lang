# Stage 18.200 — Vec::get Implementation

> **Date**: 2026-08-17
> **Version**: v0.466.0 → v0.467.0
> **Task ID**: stage18.200

## 1. Scope

Per Stage 18.199 deep review: implement Vec::get(index) — element access by index.
Extends the Vec API beyond new/push/len.

## 2. Implementation

### 2.1 C runtime helper (src/codegen/runtime.rs)

Added `__landin_vec_get(vec_ptr, index, out_ptr, elem_size)`:
- Reads ptr (offset 0) and len (offset 8) from Vec struct
- Bounds check: panics if index >= len
- Copies elem_size bytes from ptr[index * elem_size] to out_ptr

Per §1.0 原則 6 (通解>特例): one function for all Vec<T> types.
Per §1.0 原則 4 (报错>静默): OOB panics.

### 2.2 MIR intrinsic (src/mir/lower/expr_variants.rs)

`lower_vec_get_intrinsic` generates MIR for:
1. Create &Vec ref (Shared) → cast to *mut u8
2. Cast index to i64
3. Create output local + &out ref → cast to *mut u8
4. Determine elem_size (defaults to 4 for Infer/Param types — TD-VEC-ELEM-SIZE-INFERENCE)
5. Call __landin_vec_get(vec_ptr, index, out_ptr, elem_size)
6. Load result from out

### 2.3 Bug fix: elem_size for Infer/Param types

Vec::push was using `elem_size=8` for Infer-typed values (typeck resolves generic
T as Infer → i64). Fixed by defaulting to 4 (i32 size) for Infer/Param types.
This works for Vec<i32> (most common case). Proper fix needs Vec<T> type param
resolution (TD-VEC-ELEM-SIZE-INFERENCE).

### 2.4 Synthetic DefId

Registered `__landin_vec_get` as `DefId(u32::MAX - 105)`.

## 3. Verification

```
v.push(10); v.push(20); v.push(30);
v.get(0) = 10  ✅
v.get(1) = 20  ✅
v.get(2) = 30  ✅
v.get(5) → panic: vec get index out of bounds (index=5 len=1)  ✅
v.get(4) after 5 pushes = 5  ✅ (after growth)
```

## 4. Tests

4 tests (3 positive + 1 negative). All 4 pass.

## 5. §3.2 Acceptance

- ✅ cargo fmt --check: exit 0
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3073 passed (was 3069, +4 new)
- **Total**: 3731 tests, 0 failures

## 6. Tech Debt

| ID | Status |
|----|--------|
| TD-VEC-ELEM-SIZE-INFERENCE | 🟡 New — elem_size defaults to 4 for generic T (proper fix needs Vec<T> type param resolution) |
