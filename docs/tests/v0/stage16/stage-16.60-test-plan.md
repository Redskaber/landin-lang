# Stage 16.60 — Test Plan: Design Writeback + Runtime Verification

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.246.0

## 1. Test Scope

Stage 16.60 is a design writeback stage (§25.8). Tests verify:
1. Generic struct compilation + field access
2. Generic struct with methods
3. Generic enum with match
4. Nested generics (double + triple)
5. Multiple instantiations of same generic
6. No regressions on non-generic code

## 2. Test File

- `tests/v0/stage16/plan/stage16_60_design_writeback_tests.rs` — 10 tests
- All passing ✅

## 3. Integration Test Coverage (10 tests)

| # | Test | Description |
|---|------|-------------|
| 1 | `generic_struct_field_access` | Box<i32> field access |
| 2 | `generic_struct_two_params` | Pair<i32, i32> |
| 3 | `generic_struct_method` | Pair with first() method |
| 4 | `generic_enum_match` | Opt<i32> with match |
| 5 | `generic_enum_unit_variant` | Opt::None |
| 6 | `nested_generic` | Box<Box<i32>> |
| 7 | `triple_nested_generic` | Box<Box<Box<i32>>> |
| 8 | `two_instantiations` | Box<i32> + Box<bool> |
| 9 | `non_generic_no_regression` | Point struct |
| 10 | `simple_program` | fn main() { 42 } |

## 4. Runtime Verification

3 programs verified with `--run` (all exit 0):
- Box<i32> with field access
- Pair<i32, i32> with methods
- Opt<i32> enum with match

## 5. Conformance Suite

All 5224 conformance tests pass — no regressions.

## 6. References

- Stage 16.60 design: `docs/develop/v0/stage-16/stage-16.60-design-writeback.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
