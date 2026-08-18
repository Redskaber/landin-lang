# Stage 18.185 — String Intrinsics (TD-STRING-INTRINSICS)

> **Date**: 2026-08-17
> **Version**: v0.452.0 → v0.453.0
> **Task ID**: stage18.185
> **Agent**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **Depends on**: Stage 18.184 (str methods)
> **Blocks**: Stage 18.186 (format!)

## 1. Scope

Per Stage 18.181 task review: implement String intrinsics:
- `String::from_str(s: &str) -> String` — alloc + memcpy + construct
- `String::len() -> i64` — field access (via prelude impl block)
- `String::as_str() -> &str` — deferred (needs fat pointer construction from fields)
- `String::push_str(s: &str)` — deferred (needs realloc)

## 2. Dependency Audit

Per user directive (依赖与基础设施完整能力审查):
- ✅ `__landin_alloc` runtime stub (Stage 18.178)
- ✅ str::len() intrinsic pattern (Stage 18.173) — reusable for field extraction
- ✅ Fat pointer Field projection (Stage 18.174) — for extracting &str ptr/len
- ✅ Aggregate construction (struct literal) — already supported
- ✅ MIR Call terminator — already supported
- 🆕 `__landin_memcpy` — NEW runtime stub added this stage

**Conclusion**: Dependencies complete after adding `__landin_memcpy`.

## 3. Implementation

### 3.1 `__landin_memcpy` Runtime Stub (src/codegen/runtime.rs)

```c
void __landin_memcpy(void* dst, const void* src, long long n) {
    char* d = (char*)dst;
    const char* s = (const char*)src;
    for (long long i = 0; i < n; i++) {
        d[i] = s[i];
    }
}
```

Per §1.0 原則 6 (通解>特例): one memcpy for all byte copy operations.

### 3.2 `String::len()` Method (src/stdlib/prelude.rs)

Added to prelude:
```landin
impl String {
    fn len(&self) -> i64 { self.len }
}
```

This uses the existing field access + method resolution — no intrinsic needed.
Per §1.0 原則 6 (通解>特例): methods defined in prelude source, not intrinsics.

### 3.3 `String::from_str()` Intrinsic (src/mir/lower/expr_variants.rs)

Added `lower_string_from_str_intrinsic` function that generates MIR for:
1. Extract `len` from &str fat pointer (field 1)
2. Extract `data_ptr` from &str fat pointer (field 0)
3. Call `__landin_alloc(len)` → heap buffer
4. Call `__landin_memcpy(heap_buffer, data_ptr, len)` → copy bytes
5. Construct `String { ptr: heap_buffer, len, cap: len }` via Aggregate

The intrinsic is intercepted in `lower_call_expr` before the ADT ctor check,
by checking if the func path is `String::from_str` with 1 argument.

**String DefId lookup**: The String struct's DefId is looked up from HIR by
name (iterating hir.owners for a struct named "String"). This ensures the
constructed Aggregate has the correct type.

Per §1.0 原則 6 (通解>特例): one intrinsic for all String::from_str calls.
Per §2 原則 9 (正确>妥协): proper alloc+memcpy, not a stub.

### 3.4 Synthetic DefId Registration (src/driver/driver_validations.rs)

Registered `__landin_alloc` (DefId u32::MAX-100) and `__landin_memcpy`
(DefId u32::MAX-101) in `fn_name_by_def_id` so codegen can resolve the
synthetic Call terminators.

Offsets (100, 101) are well outside the BUILTIN_MACRO_NAMES range (max 28)
to avoid collision.

## 4. Verification

```
String::from_str("hello").len    → 5  ✅
String::from_str("hello").len() → 5  ✅ (via prelude impl method)
String::from_str("").len()       → 0  ✅
String::from_str("hi").len()    → 2  ✅
String::from_str("Hello, World!").len() → 13  ✅
```

## 5. Tests

### 5.1 New Tests (tests/v0/stage18/plan/stage18_185_string_intrinsics_tests.rs)

7 tests (6 positive + 1 negative):
- Positive: from_str length, len() method, from_str empty, various lengths,
  str+String methods combined, field access (ptr/len/cap)
- Negative: from_str with wrong arg type (soft)

All 7 pass.

## 6. §3.2 Acceptance

- ✅ cargo check --all-features: 0 errors / 1 warning (unused import)
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3027 passed (was 3020, +7 new)
- **Total**: 3685 tests, 0 failures

## 7. Tech Debt Status

| ID | Status |
|----|--------|
| TD-STRING-INTRINSICS | 🟡 Partial — from_str + len done; as_str + push_str deferred |
| TD-STRING-AS-STR-ALIAS | ✅ Resolved (Stage 18.180) |
| TD-HEAP-ALLOC | ✅ Resolved (Stage 18.178) |
| TD-BOX-AUTO-DROP | 🟡 Active — Box::new + auto-drop |
| TD-ARRAY-BOUNDS-CHECK | 🟡 Active — OOB not checked |

## 8. Deferred Items

- `String::as_str() -> &str` — needs fat pointer construction from fields
  (String.ptr + String.len → &str { ptr, i64 }). Requires codegen support
  for constructing a fat pointer from two separate values.
- `String::push_str(s: &str)` — needs realloc (or alloc + memcpy + free old)
- `String::new()` — trivial: String { ptr: null, len: 0, cap: 0 }
- Auto-drop for String — needs drop glue to call __landin_dealloc

## 9. Next Steps

Stage 18.186: format! macro
- Now that String::from_str works, format! can build on it:
  - format!("{} {}", a, b) → allocate result String, write formatted args
  - Needs __landin_format runtime stub or MIR intrinsic
