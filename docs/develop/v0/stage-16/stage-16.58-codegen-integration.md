# Stage 16.58 — Task 11 Phase 4c: Codegen Integration with MonoLayoutMap

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.243.0 → v0.244.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.58 implements Task 11 Phase 4c — codegen integration with per-mono
layouts. The new `mir_type_to_emit_type_with_layouts_and_mono` function first
checks `lookup_mono_layout` for generic types (non-empty substs), falling
back to the existing `AdtLayouts` map for non-generic types. This is the
final monomorphization integration step — codegen can now produce
specialized LLVM types for generic instantiations.

**What was implemented**:

1. **`lookup_mono_layout(def_id, substs, mono_layouts) -> Option<&AdtLayout>`**
   — new helper in `src/mir/monomorphize.rs`. Looks up a specialized layout
   by `(DefId, substs)` key. Returns `None` for non-generic types (empty
   substs) or when `mono_layouts` is `None`.

2. **`mir_type_to_emit_type_with_layouts_and_mono(ty, layouts, mono_layouts) -> EmitType`**
   — new codegen function in `src/codegen/mir_translation.rs`. Extends the
   existing `mir_type_to_emit_type_with_layouts` with optional per-mono
   layouts. For `TyKind::Adt(def_id, substs)`:
   - If `substs` non-empty: try `lookup_mono_layout` first
   - Fall back to `AdtLayouts` (non-generic types)
   - Recurses into Tuple, Array, Ref, Slice, Closure with `mono_layouts`

3. **`adt_layout_to_emit_type(layout, layouts, mono_layouts) -> EmitType`**
   — private helper that converts an `AdtLayout` to `EmitType`, recursing
   with `mono_layouts` so nested generic Adts resolve correctly.

4. **Re-exports**:
   - `src/mir/mod.rs`: `pub use monomorphize::{lookup_mono_layout, ...}`
   - `src/codegen/mod.rs`: `pub use mir_translation::{mir_type_to_emit_type_with_layouts_and_mono, ...}`

5. **12 integration tests** in
   `tests/v0/stage16/plan/stage16_58_codegen_integration_tests.rs` covering:
   - `lookup_mono_layout`: finds generic, returns None for non-generic/None/empty
   - `build_mono_layouts` + `lookup_mono_layout`: Box<i32>, Box<bool>, different instantiations
   - No regressions: non-generic, simple program
   - Complex patterns: Pair, nested, generic enum

**Key result**: `Box<i32>` and `Box<bool>` now have distinct specialized
layouts in the `MonoLayoutMap`. The codegen function
`mir_type_to_emit_type_with_layouts_and_mono` resolves these to distinct
`EmitType::Struct` values (field_tys: [i32] vs [bool]).

**Test results**: 8071 tests passing (343 lib + 2504 integration + 5224
conformance), 0 failures, 0 warnings. +12 new integration tests.

## 2. Design Decisions

### 2.1 Optional mono_layouts Parameter (通用 > 特解)

`mir_type_to_emit_type_with_layouts_and_mono` takes
`mono_layouts: Option<&MonoLayoutMap>`. When `None`, it behaves exactly
like the existing `mir_type_to_emit_type_with_layouts` (backward compatible).
When `Some`, it checks per-mono layouts first for generic types.

This follows §1.0 原則 6 "通用 > 特例" — one function for generic + non-generic,
with optional mono_layouts for callers that have built the map.

### 2.2 Fallback to AdtLayouts (高内聚低耦合)

For non-generic types (empty substs) or when `lookup_mono_layout` returns
`None`, the function falls back to the existing `AdtLayouts` map. This
ensures backward compatibility — existing codegen continues to work
unchanged when `mono_layouts` is `None`.

Per §16: the per-mono map is a new, separate data structure that doesn't
interfere with existing codegen paths.

### 2.3 Recursive Threading (通解 > 特解)

`mir_type_to_emit_type_with_layouts_and_mono` recurses into Tuple, Array,
Ref, Slice, and Closure with the `mono_layouts` parameter. This ensures
nested generic Adts (e.g., `Box<Box<i32>>`) resolve their specialized
layouts at every nesting level.

Per §1.0 原則 6 "通用 > 特例": one recursive function for all type kinds.

## 3. Changes

### 3.1 New Function in `src/mir/monomorphize.rs`

```rust
pub fn lookup_mono_layout<'a>(
    def_id: DefId,
    substs: &SubstsRef,
    mono_layouts: Option<&'a MonoLayoutMap>,
) -> Option<&'a AdtLayout>
```

### 3.2 New Functions in `src/codegen/mir_translation.rs`

```rust
pub fn mir_type_to_emit_type_with_layouts_and_mono(
    ty: &Ty,
    layouts: &AdtLayouts,
    mono_layouts: Option<&MonoLayoutMap>,
) -> EmitType

fn adt_layout_to_emit_type(
    layout: &AdtLayout,
    layouts: &AdtLayouts,
    mono_layouts: Option<&MonoLayoutMap>,
) -> EmitType
```

### 3.3 Re-exports

```rust
// src/mir/mod.rs
pub use monomorphize::{..., lookup_mono_layout, ...};

// src/codegen/mod.rs
pub use mir_translation::{
    mir_type_to_emit_type_with_layouts, mir_type_to_emit_type_with_layouts_and_mono,
    stdlib_type_kind_to_emit_type,
};
```

## 4. API (§23 Compliant)

| Function | Pattern | Location |
|----------|---------|----------|
| `lookup_mono_layout` | `<verb>_<noun>_<noun>` — lookup | `src/mir/monomorphize.rs` |
| `mir_type_to_emit_type_with_layouts_and_mono` | `<verb>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` | `src/codegen/mir_translation.rs` |
| `adt_layout_to_emit_type` | `<noun>_<noun>_<prep>_<noun>` — helper | `src/codegen/mir_translation.rs` |

## 5. Test Plan

12 integration tests in `tests/v0/stage16/plan/stage16_58_codegen_integration_tests.rs`.

| Category | Tests | Description |
|----------|-------|-------------|
| lookup_mono_layout | 4 | finds generic, non-generic, None map, empty substs |
| build + lookup integration | 3 | Box<i32>, Box<bool>, different instantiations |
| No regressions | 2 | non-generic, simple program |
| Complex patterns | 3 | Pair, nested, generic enum |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 343/343 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2504/2504 PASS (+12 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 8071 tests passing, 0 failures, 0 warnings.**

## 7. Version Policy

v0.243.0 → v0.244.0 (minor bump — new public API `lookup_mono_layout`,
`mir_type_to_emit_type_with_layouts_and_mono`. No existing API changes —
backward compatible.)

## 8. Next Steps (Task 11 Roadmap)

| Phase | Status | Stage | Description |
|-------|--------|-------|-------------|
| 1a | ✅ | 16.50 | `generics_of` query |
| 1b | ✅ | 16.51 | Substs propagation into `TyKind::Adt` |
| 1c | ✅ | 16.52 | Substs propagation into `AggregateKind::Adt` |
| 2 | ✅ | 16.53 | `substitute(ty, substs)` function + integration |
| 3 | ✅ | 16.54 | `collect_mono_items` — walk MIR, dedup |
| 4a | ✅ | 16.55 | Specialized naming (`mangle_ty`, `mono_item_name`) |
| 4b-pre | ✅ | 16.56 | Nested generic args resolution |
| 4b | ✅ | 16.57 | Per-mono layouts (`MonoLayoutKey`, `build_mono_layouts`) |
| 4c | ✅ | 16.58 | Codegen integration (`lookup_mono_layout`, `_and_mono`) |

**Task 11 COMPLETE** — all phases done. Monomorphization infrastructure is
fully in place: substs propagation, substitution, collection, naming,
per-mono layouts, and codegen integration.

## 9. References

- Stage 16.57 design: `docs/develop/v0/stage-16/stage-16.57-per-mono-layouts.md`
- Stage 16.56 design: `docs/develop/v0/stage-16/stage-16.56-nested-generic-args-resolution.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Type system data flow: `docs/graph/type-system/data-flow.md`
- Stage Committee process: `docs/stage-committee-process.md` §13.4 + §23
