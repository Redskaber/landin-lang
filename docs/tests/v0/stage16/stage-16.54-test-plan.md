# Stage 16.54 — Test Plan: Monomorphization Collection

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.240.0

## 1. Test Scope

Stage 16.54 implements the `collect_mono_items` function that walks MIR
bodies and collects `MonoItem { def_id, substs }` pairs. Tests verify:

1. The `MonoItem` struct works correctly (equality, accessors)
2. `collect_from_ty` correctly extracts MonoItems from all TyKind variants
3. `collect_mono_items` walks MIR bodies and deduplicates
4. Generic types in compiled programs produce MonoItems
5. Non-generic code produces no MonoItems
6. Nested generics, multiple generics, and generic enums work correctly

## 2. Test Files

- `src/mir/monomorphize.rs` — 24 unit tests (pure function tests)
- `tests/v0/stage16/plan/stage16_54_monomorphize_tests.rs` — 12 integration tests
- All passing ✅

## 3. Unit Test Coverage (24 tests in `src/mir/monomorphize.rs`)

### §1. MonoItem struct tests (5 tests)
| # | Test | Description |
|---|------|-------------|
| 1 | `mono_item_type_def_id` | Type variant def_id accessor |
| 2 | `mono_item_fn_def_id` | Fn variant def_id accessor |
| 3 | `mono_item_closure_def_id` | Closure variant def_id accessor |
| 4 | `mono_item_equality` | Same def_id + substs → equal |
| 5 | `mono_item_inequality_different_substs` | Different substs → not equal |

### §2. collect_from_ty tests (8 tests)
| # | Test | Description |
|---|------|-------------|
| 6 | `collect_from_adt_with_substs` | Adt(1, [i32]) → 1 Type item |
| 7 | `collect_from_adt_empty_substs` | Adt(1, []) → 0 items |
| 8 | `collect_from_nested_adt` | Adt(1, [Adt(1, [i32])]) → 2 items |
| 9 | `collect_from_ref_adt` | &Adt(1, [i32]) → 1 item |
| 10 | `collect_from_tuple_of_adts` | (Adt(1, [i32]), Adt(2, [bool])) → 2 items |
| 11 | `collect_from_fn_def` | FnDef(5, [i32]) → 1 Fn item |
| 12 | `collect_from_closure` | Closure(7, [i32]) → 1 Closure item |
| 13 | `collect_from_leaf_types` | i32, bool, Str, Never, Error → 0 items |

### §3. collect_mono_items tests (5 tests)
| # | Test | Description |
|---|------|-------------|
| 14 | `collect_empty_mirs` | Empty slice → 0 items |
| 15 | `collect_from_local_decls` | 2 locals with Adt types → 2 items |
| 16 | `collect_dedup` | 2 locals with same Adt → 1 item (dedup) |
| 17 | `collect_multiple_mirs` | 2 mirs with different Adts → 2 items |
| 18 | `collect_across_mirs_dedup` | 2 mirs with same Adt → 1 item (dedup) |

### §4. Statement/rvalue/terminator tests (4 tests)
| # | Test | Description |
|---|------|-------------|
| 19 | `collect_from_aggregate_statement` | Aggregate(Adt(1, [i32])) → 1 item |
| 20 | `collect_from_cast_statement` | Cast to Adt(1, [i32]) → 1 item |
| 21 | `collect_from_call_terminator` | Call with FnDef(5, [i32]) → 1 Fn item |
| 22 | `collect_from_array_aggregate` | Aggregate(Array(Adt(1, [i32]))) → 1 item |

### §5. Complex scenarios (2 tests)
| # | Test | Description |
|---|------|-------------|
| 23 | `collect_mixed_types` | Adt(1, [i32]) + Adt(2, [bool, Adt(1, [u64])]) → 3 items |
| 24 | `debug_string` | MonoItem::debug_string produces readable output |

## 4. Integration Test Coverage (12 tests)

### §1. Basic MonoItem collection (2 tests)
| # | Test | Description |
|---|------|-------------|
| 1 | `non_generic_no_mono_items` | `fn main() { 42 }` → 0 items |
| 2 | `generic_struct_produces_mono_item` | `Box<i32>` → 1 Type item |

### §2. Deduplication (1 test)
| # | Test | Description |
|---|------|-------------|
| 3 | `dedup_same_instantiation` | `Box<i32>` used twice → 1 item |

### §3. Nested generics (1 test)
| # | Test | Description |
|---|------|-------------|
| 4 | `nested_generic_produces_mono_item` | `Box<Box<i32>>` → at least 1 item |

### §4. Generic enum (2 tests)
| # | Test | Description |
|---|------|-------------|
| 5 | `generic_enum_produces_mono_items` | `Opt<i32>` → 1 Type item |
| 6 | `generic_enum_multiple_variants` | `Opt::Some` + `Opt::None` → 1 item (dedup) |

### §5. Multiple generic types (1 test)
| # | Test | Description |
|---|------|-------------|
| 7 | `multiple_generic_structs` | `Box<i32>` + `Pair<i32, bool>` → 2 items |

### §6. No regressions (3 tests)
| # | Test | Description |
|---|------|-------------|
| 8 | `non_generic_struct_no_mono_items` | `Point { x: 1, y: 2 }` → 0 items |
| 9 | `non_generic_enum_no_mono_items` | `Color::Red` → 0 items |
| 10 | `non_generic_no_mono_items` | `fn main() { 42 }` → 0 items |

### §7. MonoItem accessor tests (2 tests)
| # | Test | Description |
|---|------|-------------|
| 11 | `mono_item_def_id_accessor` | `def_id()` returns correct DefId |
| 12 | `mono_item_substs_accessor` | `substs()` returns correct substs |

## 5. Test Strategy

### 5.1 Pure Function Tests (Unit, 24 tests)

Tests in `src/mir/monomorphize.rs` use hand-crafted `Ty`, `MirBody`,
`Statement`, and `Terminator` values. They verify the collection algorithm
in isolation — no compilation, no HIR.

### 5.2 End-to-End Compilation Tests (Integration, 12 tests)

Tests in `stage16_54_monomorphize_tests.rs` use the public `compile(src)`
API, then call `collect_mono_items(&result.mirs)` and check the collected
items. They verify that generic types in real programs produce the expected
MonoItems.

### 5.3 Known Limitation: Nested Generic Args

The inner `Box<i32>` in `Box<Box<i32>>` is currently lowered as `Error`
(the AST path can't be resolved without HIR context). So the nested test
only checks for 1 item (the outer Box), not 2. This is documented in the
test and the design doc.

## 6. Conformance Suite

All 5224 conformance tests pass — no regressions.

## 7. References

- Stage 16.54 design: `docs/develop/v0/stage-16/stage-16.54-monomorphization-collection.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Stage 16.53 test plan: `docs/tests/v0/stage16/stage-16.53-test-plan.md`
