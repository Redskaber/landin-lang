# Stage 16.56 — Nested Generic Args Resolution

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.241.0 → v0.242.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.56 fixes the nested generic args limitation that was blocking Task 11
Phase 4b. Previously, `Box<Box<i32>>` lowered the inner `Box<i32>` as `Error`
because `lower_ast_ty_to_mir_ty` couldn't resolve AST paths without HIR
context. This meant nested generics only produced 1 MonoItem (the outer Box
with `[Error]` substs) instead of 2 (outer + inner).

**What was implemented**:

1. **`lookup_type_def_id_by_name(hir, name) -> Option<DefId>`** — new helper
   in `src/mir/lower/mod.rs` that scans HIR owners for a struct/enum with
   the given name. Returns the first match.

2. **`lower_ast_ty_to_mir_ty(ty, hir: Option<&HirCrate>)`** — now accepts
   optional HIR. When `Some`, AST paths are resolved to DefIds via
   `lookup_type_def_id_by_name`. When `None`, produces `Error` (same as
   before).

3. **`lower_path_generic_args(path, region_counter, hir)`** — now accepts
   optional HIR and passes it to `lower_ast_ty_to_mir_ty`.

4. **`lower_hir_ty_to_mir_ty_with_hir(ty, hir)`** — new entry point that
   passes HIR through to the region-aware lowering.

5. **`lower_hir_ty_to_mir_ty_with_regions_and_hir(ty, region_counter, hir)`**
   — new main implementation. The old `lower_hir_ty_to_mir_ty_with_regions`
   is now a wrapper that calls this with `None`.

6. **Updated call sites** in `control_flow.rs` and `field_resolution.rs` to
   use `lower_hir_ty_to_mir_ty_with_hir` with `cx.hir` for type annotations.

7. **Updated call sites** in `expr_operand.rs` to pass `cx.hir` to
   `lower_path_generic_args`.

8. **10 integration tests** in
   `tests/v0/stage16/plan/stage16_56_nested_generics_tests.rs` covering:
   - Basic nested generics (Box<Box<i32>>, Box<Box<bool>>)
   - Triple-nested generics (Box<Box<Box<i32>>>)
   - MonoItem collection for nested generics (2+ items)
   - Different inner types (4 MonoItems)
   - Pair with nested generics
   - No regressions

9. **Updated Stage 16.54 test** — the nested generic test now expects 2+
   MonoItems (was 1+ with the Error limitation).

**Key result**: `let b: Box<Box<i32>>` now produces 2 MonoItems:
`Type { Box, [Adt(Box, [i32])] }` (outer) and `Type { Box, [i32] }` (inner).
Triple-nested `Box<Box<Box<i32>>>` produces 3 MonoItems.

**Test results**: 8043 tests passing (327 lib + 2492 integration + 5224
conformance), 0 failures, 0 warnings. +10 new integration tests.

## 2. Design Decisions

### 2.1 HIR Threading (通用 > 特解)

The fix threads `hir: Option<&HirCrate>` through the type lowering functions.
When HIR is available, nested generic paths are resolved via name lookup.
When HIR is not available (test contexts), the behavior is the same as before
(produces `Error` for unresolved paths).

This follows §1.0 原則 6 "通用 > 特例" — one path resolution strategy for
all AST paths in generic args. The `Option<&HirCrate>` parameter makes HIR
access explicit without forcing all callers to provide it.

### 2.2 Name-Based Lookup (报错 > 静默)

`lookup_type_def_id_by_name` scans HIR owners for a struct/enum with the
matching name. This is O(n) per lookup (n = number of types in the crate)
but correct for the common case. Full module-path-aware resolution is future
work.

Per §1.0 原則 5 "报错 > 静默": when the name is not found, `Error` is
produced (explicit failure) rather than a dummy `Adt(DefId(0))` (silent
wrong type).

### 2.3 Backward Compatibility

The old function signatures are preserved as wrappers:
- `lower_hir_ty_to_mir_ty(ty)` → calls `lower_hir_ty_to_mir_ty_with_hir(ty, None)`
- `lower_hir_ty_to_mir_ty_with_regions(ty, region_counter)` → calls
  `lower_hir_ty_to_mir_ty_with_regions_and_hir(ty, region_counter, None)`

This means callers that don't need HIR access continue to work unchanged.
Only callers that handle type annotations (let bindings, field types) are
updated to pass HIR.

## 3. Changes

### 3.1 New Functions

```rust
// In src/mir/lower/mod.rs
fn lookup_type_def_id_by_name(hir: &HirCrate, name: Symbol) -> Option<DefId>

pub(crate) fn lower_hir_ty_to_mir_ty_with_hir(ty: &HirTy, hir: Option<&HirCrate>) -> Ty
pub(crate) fn lower_hir_ty_to_mir_ty_with_regions_and_hir(
    ty: &HirTy, region_counter: &mut u32, hir: Option<&HirCrate>
) -> Ty
```

### 3.2 Updated Functions

```rust
// Now accept hir parameter
pub(crate) fn lower_ast_ty_to_mir_ty(ty: &ast::Ty, hir: Option<&HirCrate>) -> Ty
pub(crate) fn lower_path_generic_args(
    path: &HirPath, region_counter: &mut u32, hir: Option<&HirCrate>
) -> SubstsRef
```

### 3.3 Updated Call Sites

- `control_flow.rs` lines 596, 671: `lower_hir_ty_to_mir_ty` →
  `lower_hir_ty_to_mir_ty_with_hir` with `cx.hir`
- `field_resolution.rs` line 71: `lower_hir_ty_to_mir_ty` →
  `lower_hir_ty_to_mir_ty_with_hir` with `Some(hir)`
- `expr_operand.rs` lines 438, 470, 1875, 1902: `lower_path_generic_args`
  now passes `cx.hir`

## 4. API (§23 Compliant)

| Function | Pattern | Location |
|----------|---------|----------|
| `lookup_type_def_id_by_name` | `<verb>_<noun>_<noun>_<prep>_<noun>` | `src/mir/lower/mod.rs` |
| `lower_hir_ty_to_mir_ty_with_hir` | `<verb>_<noun>_<noun>_<prep>_<noun>` | `src/mir/lower/mod.rs` |
| `lower_hir_ty_to_mir_ty_with_regions_and_hir` | `<verb>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` | `src/mir/lower/mod.rs` |

## 5. Test Plan

10 integration tests in `tests/v0/stage16/plan/stage16_56_nested_generics_tests.rs`.

| # | Test | Description |
|---|------|-------------|
| 1 | `nested_generic_box_box_i32` | Box<Box<i32>> compiles |
| 2 | `nested_generic_box_box_bool` | Box<Box<bool>> compiles |
| 3 | `triple_nested_generic` | Box<Box<Box<i32>>> compiles |
| 4 | `nested_generic_produces_two_mono_items` | Box<Box<i32>> → 2+ MonoItems |
| 5 | `triple_nested_produces_three_mono_items` | Box<Box<Box<i32>>> → 3+ MonoItems |
| 6 | `nested_different_inner_types` | Box<Box<i32>> + Box<Box<bool>> → 4+ MonoItems |
| 7 | `nested_generic_with_pair` | Pair<Box<i32>, bool> compiles |
| 8 | `nested_generic_pair_of_boxes` | Pair<Box<i32>, Box<bool>> → 3+ MonoItems |
| 9 | `non_nested_generic_no_regression` | Box<i32> still works |
| 10 | `non_generic_no_regression` | Non-generic code still works |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 327/327 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2492/2492 PASS (+10 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 8043 tests passing, 0 failures, 0 warnings.**

## 7. Version Policy

v0.241.0 → v0.242.0 (minor bump — new functions + behavior change: nested
generic paths are now resolved correctly. This changes MIR output for nested
generics, producing correct MonoItems instead of Error.)

## 8. Next Steps (Task 11 Roadmap)

| Phase | Status | Stage | Description |
|-------|--------|-------|-------------|
| 1a | ✅ | 16.50 | `generics_of` query |
| 1b | ✅ | 16.51 | Substs propagation into `TyKind::Adt` |
| 1c | ✅ | 16.52 | Substs propagation into `AggregateKind::Adt` |
| 2 | ✅ | 16.53 | `substitute(ty, substs)` function + integration |
| 3 | ✅ | 16.54 | `collect_mono_items` — walk MIR, dedup |
| 4a | ✅ | 16.55 | Specialized naming (`mangle_ty`, `mono_item_name`) |
| 4b-pre | ✅ | 16.56 | Nested generic args resolution (prerequisite for 4b) |
| 4b | 🔧 Next | — | Layouts keyed by (DefId, SubstsRef) |
| 4c | 🔧 Planned | — | Emit specialized function definitions |

## 9. References

- Stage 16.55 design: `docs/develop/v0/stage-16/stage-16.55-per-mono-codegen-naming.md`
- Stage 16.54 design: `docs/develop/v0/stage-16/stage-16.54-monomorphization-collection.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Type system data flow: `docs/graph/type-system/data-flow.md`
- Stage Committee process: `docs/stage-committee-process.md` §13.4 + §23
