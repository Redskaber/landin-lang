# Stage 18.198 — String::push_str (TD-STRING-INTRINSICS resolved)

> **Date**: 2026-08-17
> **Version**: v0.464.0 → v0.465.0
> **Task ID**: stage18.198

## 1. Scope

Per Stage 18.196 deep review: implement String::push_str — append &str to owned String.
This resolves TD-STRING-INTRINSICS (push_str was the last deferred String method).

## 2. Implementation

### 2.1 C runtime helper (src/codegen/runtime.rs)

Added `__landin_string_push_str(str_ptr, src_ptr, src_len)`:
- Reads len (offset 8) and cap (offset 16) from String struct
- Grows capacity if needed (cap 0→4, else 2× until >= new_len)
- Copies src bytes to ptr[len]
- Updates len

Per §1.0 原則 6 (通解>特例): one function for all String::push_str calls.
Per §1.0 原則 4 (报错>静默): OOM panics.

### 2.2 MIR intrinsic (src/mir/lower/expr_variants.rs)

`lower_string_push_str_intrinsic` generates MIR for:
1. Create &String ref (Shared) → cast to *mut u8
2. Extract src.ptr (field 0) and src.len (field 1) from &str fat pointer
3. Call __landin_string_push_str(str_ptr, src_ptr, src_len)

Same pattern as Vec::push (Stage 18.197): Shared borrow + Cast to opaque pointer.

### 2.3 Synthetic DefId

Registered `__landin_string_push_str` as `DefId(u32::MAX - 104)`.

## 3. Verification

```
String::from_str("hello").push_str(" world").len() = 11  ✅
String::new().push_str("hello").len() = 5                 ✅
String::new() + push_str("Hello") + push_str(", ") + push_str("World!") → len=13  ✅
Growth: 3 pushes → len=13, cap=16 (0→4→8→16)              ✅
push_str("") on existing → len unchanged                   ✅
push_str("The quick brown fox...") → len=43                ✅
```

## 4. Tests

6 tests (all positive). All 6 pass.

## 5. §3.2 Acceptance

- ✅ cargo fmt --check: exit 0
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3069 passed (was 3063, +6 new)
- **Total**: 3727 tests, 0 failures

## 6. Tech Debt

| ID | Status |
|----|--------|
| TD-STRING-INTRINSICS | ✅ Resolved — from_str + new + len + as_str + push_str all done |
