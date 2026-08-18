# Stage 18.194 — Realloc Infrastructure (TD-VEC-MVP prerequisite)

> **Date**: 2026-08-17
> **Version**: v0.460.0 → v0.461.0
> **Task ID**: stage18.194

## 1. Scope

Per Stage 18.191 task review: implement `__landin_realloc` runtime stub.
This is the prerequisite for Vec<T> (Stage 18.195) and String::push_str
(Stage 18.196), both of which need dynamic buffer growth.

## 2. Dependency Audit

Per user directive (依赖与基础设施完整能力审查):
- ✅ `__landin_alloc` (Stage 18.178)
- ✅ `__landin_dealloc` (Stage 18.178)
- ✅ `__landin_memcpy` (Stage 18.185)
- ✅ C wrapper `#include <stdlib.h>` (libc realloc available)
- ✅ Synthetic DefId registration pattern (Stage 18.185)

**Conclusion**: All dependencies complete. Realloc wraps libc `realloc`.

## 3. Implementation

### 3.1 C Wrapper Stub (src/codegen/runtime.rs)

```c
void* __landin_realloc(void* ptr, long long old_size, long long new_size) {
    void* new_ptr = realloc(ptr, (size_t)new_size);
    if (new_ptr == 0) {
        fprintf(stderr, "panic: memory reallocation failed (old=%lld new=%lld)\n", old_size, new_size);
        exit(1);
    }
    return new_ptr;
}
```

Per §1.0 原則 6 (通解>特例): one realloc for all heap growth operations.
Per §1.0 原則 4 (报错>静默): OOM must panic, not return NULL.

### 3.2 Synthetic DefId Registration (src/driver/driver_validations.rs)

Registered `__landin_realloc` as `DefId(u32::MAX - 102)` in
`fn_name_by_def_id`, following the Stage 18.185 pattern (alloc=100,
memcpy=101, realloc=102).

### 3.3 Runtime Test (src/codegen/runtime.rs)

Updated `stage18_157_c_wrapper_contains_all_stubs` to include
`__landin_realloc` in the required symbols list.

## 4. Verification

```
realloc(ptr, 4, 8) preserves *ptr=42 → 42  ✅
realloc(ptr, 8, 4) preserves *ptr=99 → 99  ✅
realloc chain: alloc(2) → realloc(2,4) → realloc(4,8) → preserves  ✅
realloc(NULL, 0, 4) behaves like alloc → 7  ✅
```

## 5. Tests (tests/v0/stage18/plan/stage18_194_realloc_tests.rs)

4 tests (all positive):
- realloc preserves data on grow
- realloc preserves data on shrink
- realloc chain (multiple reallocs)
- realloc NULL (behaves like alloc)

All 4 pass.

## 6. §3.2 Acceptance

- ✅ cargo check --all-features: 0 errors / 1 warning
- ✅ cargo fmt --check: exit 0
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3053 passed (was 3049, +4 new)
- **Total**: 3711 tests, 0 failures

## 7. Tech Debt Status

| ID | Status |
|----|--------|
| TD-VEC-MVP | 🟡 Unblocked — realloc now available, Vec can be implemented |
| TD-STRING-INTRINSICS | 🟡 Unblocked — String::push_str can use realloc |
