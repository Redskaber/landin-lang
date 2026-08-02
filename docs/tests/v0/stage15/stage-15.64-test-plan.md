# Stage 15.64 — Test Plan: Struct Literal Copy→Move + Field-Copy Drop Prevention

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.189.0 → v0.190.0
> **Process**: stage-committee-process.md v3.23 §17.5

## 1. Test Categories

### 1.1 Regression — Conformance suite (5216 tests)
**Expected**: All 5216 pass (no regression).

### 1.2 New — Integration tests (8 tests)
**File**: `tests/v0/stage15/plan/struct_literal_copy_move_tests.rs`

| Test | Pattern | What it verifies |
|------|---------|------------------|
| `stage15_64_struct_literal_non_copy_field_no_double_drop` | Outer+Inner both Drop | Struct literal Move for non-Copy field |
| `stage15_64_field_access_no_double_drop` | Outer (no Drop) + Inner (Drop) | Field-copy temp not dropped |
| `stage15_64_nested_struct_literals_no_double_drop` | 3-level nesting | All temps moved, no double-drop |
| `stage15_64_struct_literal_copy_fields_no_regression` | Point {x:i32, y:i32} | Copy fields still use Copy |
| `stage15_64_field_access_non_drop_no_regression` | Point (no Drop) | Non-Drop struct field access |
| `stage15_64_multiple_field_accesses_no_double_drop` | o.inner.x + o.inner.y | Multiple field-copy temps |
| `stage15_64_struct_literal_mixed_copy_non_copy` | Mixed {i32, Inner} | Copy + Move in same literal |
| `stage15_64_function_return_struct_with_drop_field` | fn returns Outer | Cross-function no double-drop |

### 1.3 Runtime — Drop count verification (manual)
**Expected**: 2 drops (outer + inner recursive), not 4.

## 2. Sign-off
- ✅ All 5216 conformance tests pass.
- ✅ All 2126 integration tests pass (including 8 new).
- ✅ All 226 lib tests pass.
- ✅ 0 clippy warnings, fmt clean.
- ✅ Runtime: 2 drops (correct), not 4.

**Total: 7568 tests passing, 0 failures.**

Stage 15.64 is GO for merge.
