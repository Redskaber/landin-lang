# Stage 16.57 — Test Plan: Per-Mono Layouts

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.243.0

## 1. Test Scope

Stage 16.57 implements per-mono layouts (`MonoLayoutKey`, `MonoLayoutMap`,
`build_mono_layouts`). Tests verify:

1. `MonoLayoutKey` correctly wraps (DefId, Vec<TyKind>) and is hashable
2. `build_mono_layouts` builds correct layouts for generic types
3. Different instantiations produce different layouts
4. Same instantiation used twice produces one layout (dedup)
5. Field types are correctly substituted (not Param or Error)
6. Nested generics produce nested layouts

## 2. Test File

- `src/mir/monomorphize.rs` — 16 unit tests (Phase 4b)
- All passing ✅

## 3. Unit Test Coverage (16 tests)

### §9. MonoLayoutKey tests (8 tests)
| # | Test | Description |
|---|------|-------------|
| 1 | `mono_layout_key_new` | Create from DefId + substs, extract TyKind |
| 2 | `mono_layout_key_empty_substs` | Empty substs → empty key.substs |
| 3 | `mono_layout_key_equality` | Same DefId + substs → equal |
| 4 | `mono_layout_key_inequality_different_def_id` | Different DefId → not equal |
| 5 | `mono_layout_key_inequality_different_substs` | Different substs → not equal |
| 6 | `mono_layout_key_from_mono_item_type` | From MonoItem::Type |
| 7 | `mono_layout_key_from_mono_item_fn` | From MonoItem::Fn |
| 8 | `mono_layout_key_hashable` | HashSet dedup works |

### §10. build_mono_layouts tests (8 tests)
| # | Test | Description |
|---|------|-------------|
| 9 | `build_mono_layouts_empty_items` | Empty items → empty map |
| 10 | `build_mono_layouts_non_generic_skipped` | Empty substs → skipped |
| 11 | `build_mono_layouts_generic_struct` | Box<i32> → 1 layout |
| 12 | `build_mono_layouts_two_instantiations` | Box<i32> + Box<bool> → 2 layouts |
| 13 | `build_mono_layouts_dedup` | Box<i32> used twice → 1 layout |
| 14 | `build_mono_layouts_nested_generic` | Box<Box<i32>> → 2+ layouts |
| 15 | `build_mono_layouts_correct_field_type` | Box<i32> field is i32 (substituted) |
| 16 | `build_mono_layouts_generic_enum` | Opt<i32> → enum layout |

## 4. Test Strategy

### 4.1 Pure Unit Tests (Tests 1-8)

Tests 1-8 use hand-crafted `Ty` and `MonoItem` values. They verify
`MonoLayoutKey` in isolation — no compilation, no HIR.

### 4.2 End-to-End Layout Building Tests (Tests 9-16)

Tests 9-16 compile a source string, collect MonoItems, call
`build_mono_layouts`, and verify the resulting map. They confirm that:
- Generic types produce layouts with substituted field types
- Different instantiations produce different layouts
- Same instantiation deduplicates
- Nested generics produce nested layouts
- Field types are concrete (not Param or Error)

## 5. Conformance Suite

All 5224 conformance tests pass — no regressions.

## 6. References

- Stage 16.57 design: `docs/develop/v0/stage-16/stage-16.57-per-mono-layouts.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Stage 16.56 test plan: `docs/tests/v0/stage16/stage-16.56-test-plan.md`
