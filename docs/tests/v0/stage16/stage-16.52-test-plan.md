# Stage 16.52 — Test Plan: AggregateKind::Adt Substs Propagation

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.238.0

## 1. Test Scope

Stage 16.52 propagates generic args into `AggregateKind::Adt` at all 5
construction sites in `src/mir/lower/expr_operand.rs`. Tests verify:

1. Generic struct literals compile and unify with annotated types
2. Generic enum variants (tuple, struct, unit) compile and unify
3. Generic enums work in return position and match scrutinee
4. MIR is well-formed and substs propagate to local decls
5. No regressions on non-generic code
6. Empty substs unify with non-empty substs (inference edge case)

## 2. Test File

- `tests/v0/stage16/plan/stage16_52_aggregate_substs_tests.rs` (15 tests)
- All passing ✅

## 3. Test Coverage Matrix

### §1. Generic struct literal compilation
| # | Test | Description |
|---|------|-------------|
| 1 | `generic_struct_literal_unifies` | `Pair<i32, i32>` annot + `Pair { ... }` literal |
| 2 | `generic_struct_literal_inferred` | Inference case (no annot) — verifies no panic |
| 3 | `single_param_generic_struct` | `Box<i32>` annot + `Box { val: 42 }` literal |

### §2. Generic enum variant compilation
| # | Test | Description |
|---|------|-------------|
| 4 | `generic_enum_tuple_variant_unifies` | `Opt::Some(42)` with `Opt<i32>` annot |
| 5 | `generic_enum_unit_variant_unifies` | `Opt::None` with `Opt<i32>` annot |
| 6 | `generic_enum_struct_variant_unifies` | `Shape::Circle { r: 1 }` with `Shape<i32>` annot |

### §3. Return-type generic
| # | Test | Description |
|---|------|-------------|
| 7 | `generic_enum_return` | `fn make() -> Opt<i32> { Opt::Some(42) }` |
| 8 | `generic_enum_in_match` | Match scrutinee with generic enum |

### §4. MIR substs propagation verification
| # | Test | Description |
|---|------|-------------|
| 9 | `aggregate_substs_propagated_in_mir` | MIR is built, no internal errors |
| 10 | `type_annotation_substs_in_local_decl` | Local decl has `Adt(def, [subst])` |

### §5. No regressions on non-generic code
| # | Test | Description |
|---|------|-------------|
| 11 | `non_generic_struct_no_regression` | Non-generic struct still compiles |
| 12 | `non_generic_enum_no_regression` | Non-generic enum still compiles |
| 13 | `non_generic_enum_with_data_no_regression` | Non-generic enum w/ data still compiles |

### §6. Typeck unification (Phase 1c edge case)
| # | Test | Description |
|---|------|-------------|
| 14 | `empty_substs_unify_with_non_empty` | `Opt::None` annot `Opt<i32>` via fn return |
| 15 | `document_substs_mismatch_intent` | Forward-looking doc test for Phase 2 |

## 4. Test Strategy

### 4.1 Black-Box Compilation Tests (Tests 1-8, 11-14)

These tests use the public `compile(src: &str) -> CompileResult` API
and check `result.has_errors()`. They verify end-to-end compilation
succeeds for various generic construct patterns.

### 4.2 MIR Inspection Tests (Tests 9-10)

These tests go beyond black-box compilation by inspecting the resulting
`MirBody` to verify substs are correctly propagated into:
- Local declarations (test 10)
- Overall MIR well-formedness (test 9)

### 4.3 Edge Case Tests (Tests 14-15)

Test 14 verifies the "empty substs unify with non-empty" rule from
Stage 16.52's unify.rs rework. This is the inference edge case where
type annotations have substs but path expressions don't (until Phase 2
adds back-propagation).

Test 15 is a forward-looking documentation test — it doesn't verify
behavior today but documents the intent that mismatched non-empty
substs should error once Phase 2 (substitution) lands.

## 5. Conformance Suite

All 5224 conformance tests pass — no regressions on:
- 04-e2e/03-closures (28 tests)
- 04-e2e/04-error-handling (28 tests)
- 04-e2e/06-run-ok (171 tests)
- Other conformance directories

## 6. References

- Stage 16.52 design: `docs/develop/v0/stage-16/stage-16.52-aggregate-substs-propagation.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Stage 16.51 test plan: `docs/tests/v0/stage16/stage-16.51-test-plan.md`
