# Stage 18.184 — str Methods Runtime Fix (TD-STR-METHODS-RUNTIME)

> **Date**: 2026-08-17
> **Version**: v0.451.0 → v0.452.0
> **Task ID**: stage18.184
> **Agent**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **Depends on**: Stage 18.183 (fat pointer Index)
> **Blocks**: Stage 18.185 (String intrinsics)

## 1. Scope

Per Stage 18.181 task review: fix str methods that compile but segfault at
runtime:
- `s.is_empty()` → segfault (wrong method resolution → recursive call)
- `s.as_bytes()` → segfault (same root cause)
- `s.to_string()` → deferred (needs String intrinsics, Stage 18.185)

## 2. Dependency Audit

Per user directive (依赖与基础设施完整能力审查):
- ✅ str::len() intrinsic pattern (Stage 18.173) — reusable
- ✅ Fat pointer Field projection (Stage 18.174) — reusable for len extraction
- ✅ BinaryOp Eq for bool comparison — already supported
- ✅ Fat pointer Index projection (Stage 18.183) — for as_bytes()[N]
- ✅ MIR lower method call interception — str::len() precedent

**Conclusion**: Dependencies complete, no re-plan needed.

## 3. Root Cause

### 3.1 Bug Reproduction

```landin
let s: &str = "hello";
s.is_empty();  // Segfault at runtime
```

### 3.2 Root Cause

`is_empty` and `as_bytes` are NOT defined in the prelude (only `is_some`/`is_none`
for Option). The resolver fell through to wrong method resolution — calling
`landin_main` recursively → infinite recursion → segfault.

### 3.3 The Fix

Added MIR intrinsics for str methods, following the str::len() pattern:

**`s.is_empty()`** → `s.len() == 0` (returns bool):
1. Extract len field (FieldId(1)) — same as str::len() intrinsic
2. Compare len == 0 via BinaryOp::Eq → bool

**`s.as_bytes()`** → return the receiver directly (no-op):
- &str and &[u8] have the SAME LLVM layout (`{ ptr, i64 }`)
- Just return the receiver local — the fat pointer IS the &[u8]

Per §1.0 原則 6 (通解>特例): reuse str::len() Field projection pattern.
Per §1.0 原則 9 (正确>妥协): is_empty is semantically `len == 0`, not a stub.

## 4. Verification

```
"hello".is_empty() → false  ✅ (was: segfault)
"".is_empty()      → true   ✅ (was: segfault)
"hello".as_bytes().len() → 5  ✅ (was: segfault)
"hello".as_bytes()[0] → 104 ('h')  ✅
"hello".as_bytes()[4] → 111 ('o')  ✅
```

## 5. Tests

### 5.1 New Tests (tests/v0/stage18/plan/stage18_184_str_methods_tests.rs)

8 tests (7 positive + 1 negative):
- Positive: is_empty non-empty, is_empty empty, is_empty multiple,
  as_bytes length, as_bytes index, len+is_empty combined, long string
- Negative: is_empty with wrong arg count (soft)

All 8 pass.

## 6. §3.2 Acceptance

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3020 passed (was 3012, +8 new)
- **Total**: 3678 tests, 0 failures

## 7. Tech Debt Status

| ID | Status |
|----|--------|
| TD-STR-METHODS-RUNTIME | ✅ Resolved (Stage 18.184) — is_empty + as_bytes work |
| TD-STRING-INTRINSICS | 🟡 Active — Stage 18.185 (from_str/push_str/len/as_str) |
| TD-TO-STRING | 🟡 New — to_string needs String intrinsics (Stage 18.185) |

## 8. Next Steps

Stage 18.185: String intrinsics (from_str/push_str/len/as_str)
- Now that str methods work, String intrinsics can build on them:
  - String::from_str(s: &str) → alloc + copy bytes + wrap in String struct
  - String::len() → access String.len field
  - String::as_str() → construct &str fat pointer from String.ptr + String.len
  - String::push_str(s: &str) → realloc + copy (deferred — needs realloc support)
