# Stage 15.66 — Test Plan: Recursive Drop for Enums

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.191.0 → v0.192.0
> **Process**: stage-committee-process.md v3.23 §17.5

## 1. Test Categories

### 1.1 Regression — Conformance suite (5216 tests)
**Expected**: All 5216 pass (no regression from enum drop glue changes).

### 1.2 New — Integration tests (8 tests)
**File**: `tests/v0/stage15/plan/enum_recursive_drop_tests.rs`

| Test | Pattern | What it verifies |
|------|---------|------------------|
| `stage15_66_enum_no_drop_impl_variant_has_drop` | E(Inner) no impl Drop | Variant payload recursively dropped |
| `stage15_66_enum_has_drop_impl_and_variant_has_drop` | E(Inner) with impl Drop | User drop + variant payload drop |
| `stage15_66_enum_multiple_drop_variants` | E(A, B) both Drop | SwitchInt with 2 cases |
| `stage15_66_enum_no_drop_variants_no_regression` | E(i32, bool) | No SwitchInt emitted |
| `stage15_66_enum_mixed_drop_non_drop_variants` | E(Inner, i32) | Only Drop variant gets case |
| `stage15_66_enum_drop_runtime_verification` | fn returns E | Runtime: variant payload dropped |
| `stage15_66_nested_enum_in_struct_recursive_drop` | Outer { e: E } | Struct drop calls enum drop |
| `stage15_66_enum_struct_variant_payload_drop` | E::A { inner: Inner } | Struct variant payload dropped |

### 1.3 Runtime — Drop order verification (manual)
**Expected**: "enum dropped" then "inner dropped" (user drop first, then variant payload).

## 2. Sign-off
- ✅ All 5216 conformance tests pass.
- ✅ All 2133 integration tests pass (including 8 new enum drop tests).
- ✅ All 226 lib tests pass.
- ✅ 0 clippy warnings, fmt clean.
- ✅ Runtime: correct drop order (enum user drop, then variant payload).

**Total: 7575 tests passing, 0 failures.**

Stage 15.66 is GO for merge.
