# Stage 16.55 — Test Plan: Per-Mono Codegen Specialized Naming

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.241.0

## 1. Test Scope

Stage 16.55 implements the specialized naming scheme for monomorphized
items. Tests verify:

1. `mangle_ty` correctly mangles all TyKind variants
2. `mangle_ty_with_interner` resolves Adt names via interner
3. `mono_item_name` generates specialized names for MonoItems
4. `build_mono_item_names` builds the full name map

## 2. Test File

- `src/mir/monomorphize.rs` — 24 unit tests (Phase 3 + Phase 4)
- All passing ✅

## 3. Unit Test Coverage (24 Phase 4 tests)

### §6. mangle_ty tests (16 tests)
| # | Test | Description |
|---|------|-------------|
| 1 | `mangle_ty_bool` | Bool → "bool" |
| 2 | `mangle_ty_i32` | I32 → "i32" |
| 3 | `mangle_ty_adt_with_substs` | Adt(5, [i32]) → "Adt_5_i32" |
| 4 | `mangle_ty_adt_empty_substs` | Adt(5, []) → "Adt_5" |
| 5 | `mangle_ty_ref` | &i32 → "ref_i32" |
| 6 | `mangle_ty_ref_mut` | &mut i32 → "refmut_i32" |
| 7 | `mangle_ty_tuple` | (i32, bool) → "tuple_i32_bool" |
| 8 | `mangle_ty_empty_tuple` | () → "unit" |
| 9 | `mangle_ty_array` | [i32; 10] → "array_i32_10" |
| 10 | `mangle_ty_slice` | [i32] → "slice_i32" |
| 11 | `mangle_ty_nested_adt` | Adt(2, [Adt(1, [i32])]) → "Adt_2_Adt_1_i32" |
| 12 | `mangle_ty_fn_def` | FnDef(7, [i32]) → "fn_7_i32" |
| 13 | `mangle_ty_closure` | Closure(3, [i32]) → "closure_3_i32" |
| 14 | `mangle_ty_param` | Param(0) → "param_0" |
| 15 | `mangle_ty_str` | Str → "str" |
| 16 | `mangle_ty_never` | Never → "never" |

### §7. mono_item_name tests (5 tests)
| # | Test | Description |
|---|------|-------------|
| 17 | `mono_item_name_type_with_substs` | Type{Box, [i32]} + "Box" → "Box_i32" |
| 18 | `mono_item_name_fn_with_substs` | Fn{id, [i32]} + "id" → "id_i32" |
| 19 | `mono_item_name_empty_substs` | Type{Box, []} + "Box" → "Box" |
| 20 | `mono_item_name_multiple_substs` | Type{Pair, [i32, bool]} + "Pair" → "Pair_i32_bool" |
| 21 | `mono_item_name_nested_substs` | Type{Outer, [Adt(1, [i32])]} + "Outer" → "Outer_Adt_1_i32" |

### §8. build_mono_item_names tests (3 tests)
| # | Test | Description |
|---|------|-------------|
| 22 | `build_mono_item_names_basic` | 2 Fn items → "id_i32", "id_bool" |
| 23 | `build_mono_item_names_empty` | Empty items → empty map |
| 24 | `build_mono_item_names_mixed` | Fn + Type + Closure → 3 distinct names |

## 4. Test Strategy

### 4.1 Pure Function Tests (Unit, 24 tests)

All tests use hand-crafted `Ty` and `MonoItem` values. They verify the
naming functions in isolation — no compilation, no HIR, no interner (except
for `mono_item_name` and `build_mono_item_names` which take an interner
parameter for Symbol resolution).

### 4.2 Mangling Rules

The mangling scheme is:
- Leaf types: lowercase name (`i32`, `bool`, `str`, etc.)
- `Ref`: `ref_` or `refmut_` + inner
- `RawPtr`: `ptr_` or `ptrmut_` + inner
- `Array`: `array_` + inner + `_` + length
- `Slice`: `slice_` + inner
- `Tuple`: `tuple_` + elements joined by `_` (or `unit` for empty)
- `Adt`: `Adt_` + DefId (fallback) or resolved name + `_` + substs
- `FnDef`: `fn_` + DefId + `_` + substs
- `Closure`: `closure_` + DefId + `_` + substs
- `FnPtr`: `fnptr_` + inputs + `__` + output
- `Param`: `param_` + index
- `Infer`: `infer`

## 5. Conformance Suite

All 5224 conformance tests pass — no regressions.

## 6. References

- Stage 16.55 design: `docs/develop/v0/stage-16/stage-16.55-per-mono-codegen-naming.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Stage 16.54 test plan: `docs/tests/v0/stage16/stage-16.54-test-plan.md`
