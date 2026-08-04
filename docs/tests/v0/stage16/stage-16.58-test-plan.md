# Stage 16.58 — Test Plan: Codegen Integration with MonoLayoutMap

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.244.0

## 1. Test Scope

Stage 16.58 integrates per-mono layouts into the codegen type translation
pipeline. Tests verify:

1. `lookup_mono_layout` finds specialized layouts for generic types
2. `lookup_mono_layout` returns None for non-generic types, None map, empty substs
3. `build_mono_layouts` + `lookup_mono_layout` work together
4. Different instantiations produce different layouts
5. No regressions on non-generic code
6. Complex generic patterns (Pair, nested, enum) work correctly

## 2. Test File

- `tests/v0/stage16/plan/stage16_58_codegen_integration_tests.rs` — 12 tests
- All passing ✅

## 3. Integration Test Coverage (12 tests)

### §1. lookup_mono_layout tests (4 tests)
| # | Test | Description |
|---|------|-------------|
| 1 | `lookup_mono_layout_finds_generic` | Generic type → Some(layout) |
| 2 | `lookup_mono_layout_non_generic` | Non-generic → None |
| 3 | `lookup_mono_layout_none_map` | None map → None |
| 4 | `lookup_mono_layout_empty_substs` | Empty substs → None |

### §2. build_mono_layouts + lookup integration (3 tests)
| # | Test | Description |
|---|------|-------------|
| 5 | `box_i32_layout_has_i32_field` | Box<i32> → field_tys: [i32] |
| 6 | `box_bool_layout_has_bool_field` | Box<bool> → field_tys: [bool] |
| 7 | `different_instantiations_different_layouts` | Box<i32> + Box<bool> → 2 distinct layouts |

### §3. No regressions (2 tests)
| # | Test | Description |
|---|------|-------------|
| 8 | `non_generic_no_regression` | Non-generic → 0 mono layouts |
| 9 | `simple_program_no_regression` | `fn main() { 42 }` works |

### §4. Complex generic patterns (3 tests)
| # | Test | Description |
|---|------|-------------|
| 10 | `pair_layout_two_fields` | Pair<i32, bool> → [i32, bool] |
| 11 | `nested_generic_layouts` | Box<Box<i32>> → 2+ layouts |
| 12 | `generic_enum_layout` | Opt<i32> → enum layout |

## 4. Test Strategy

### 4.1 lookup_mono_layout Tests (Tests 1-4)

These tests verify the lookup helper in isolation. They check all code
paths: Some/None map, empty/non-empty substs, generic/non-generic types.

### 4.2 End-to-End Layout Building + Lookup (Tests 5-7)

These tests compile a source string, build mono layouts, then look up
specific layouts and verify their field types. They confirm that:
- Generic types produce layouts with substituted field types
- Different instantiations produce different layouts

### 4.3 No-Regression Tests (Tests 8-9)

These tests verify that non-generic code still works correctly and
produces no mono layouts.

### 4.4 Complex Pattern Tests (Tests 10-12)

These tests verify complex generic patterns: two type params (Pair),
nested generics (Box<Box<i32>>), and generic enums (Opt<i32>).

## 5. Conformance Suite

All 5224 conformance tests pass — no regressions.

## 6. References

- Stage 16.58 design: `docs/develop/v0/stage-16/stage-16.58-codegen-integration.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Stage 16.57 test plan: `docs/tests/v0/stage16/stage-16.57-test-plan.md`
