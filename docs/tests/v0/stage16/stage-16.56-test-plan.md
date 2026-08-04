# Stage 16.56 — Test Plan: Nested Generic Args Resolution

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.242.0

## 1. Test Scope

Stage 16.56 fixes the nested generic args limitation. Tests verify:

1. Nested generics compile without errors (Box<Box<i32>>, etc.)
2. Triple-nested generics compile (Box<Box<Box<i32>>>)
3. Nested generics produce correct MonoItem counts
4. Different inner types produce distinct MonoItems
5. Pair with nested generics works
6. No regressions on non-nested and non-generic code

## 2. Test File

- `tests/v0/stage16/plan/stage16_56_nested_generics_tests.rs` — 10 tests
- All passing ✅

## 3. Integration Test Coverage (10 tests)

### §1. Nested generics — basic compilation (3 tests)
| # | Test | Description |
|---|------|-------------|
| 1 | `nested_generic_box_box_i32` | Box<Box<i32>> compiles |
| 2 | `nested_generic_box_box_bool` | Box<Box<bool>> compiles |
| 3 | `triple_nested_generic` | Box<Box<Box<i32>>> compiles |

### §2. Nested generics — MonoItem collection (2 tests)
| # | Test | Description |
|---|------|-------------|
| 4 | `nested_generic_produces_two_mono_items` | Box<Box<i32>> → 2+ MonoItems |
| 5 | `triple_nested_produces_three_mono_items` | Box<Box<Box<i32>>> → 3+ MonoItems |

### §3. Nested generics with different inner types (1 test)
| # | Test | Description |
|---|------|-------------|
| 6 | `nested_different_inner_types` | Box<Box<i32>> + Box<Box<bool>> → 4+ MonoItems |

### §4. Nested generics with Pair (2 tests)
| # | Test | Description |
|---|------|-------------|
| 7 | `nested_generic_with_pair` | Pair<Box<i32>, bool> compiles |
| 8 | `nested_generic_pair_of_boxes` | Pair<Box<i32>, Box<bool>> → 3+ MonoItems |

### §5. No regressions (2 tests)
| # | Test | Description |
|---|------|-------------|
| 9 | `non_nested_generic_no_regression` | Box<i32> still works |
| 10 | `non_generic_no_regression` | Non-generic code → 0 MonoItems |

## 4. Updated Stage 16.54 Test

The Stage 16.54 test `stage16_54_nested_generic_produces_nested_mono_items`
(formerly `stage16_54_nested_generic_produces_mono_item`) has been updated
to expect 2+ MonoItems instead of 1+. This reflects the fix — nested
generics now produce both outer and inner MonoItems.

## 5. Test Strategy

### 5.1 End-to-End Compilation Tests (Tests 1-3, 7, 9-10)

These tests use `compile(src)` and check `result.has_errors()`. They verify
that nested generic types compile successfully end-to-end.

### 5.2 MonoItem Collection Tests (Tests 4-6, 8)

These tests compile a program, then call `collect_mono_items(&result.mirs)`
and verify the MonoItem count. They confirm that nested generics produce
the expected number of MonoItems (outer + inner).

## 6. Conformance Suite

All 5224 conformance tests pass — no regressions.

## 7. References

- Stage 16.56 design: `docs/develop/v0/stage-16/stage-16.56-nested-generic-args-resolution.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Stage 16.55 test plan: `docs/tests/v0/stage16/stage-16.55-test-plan.md`
