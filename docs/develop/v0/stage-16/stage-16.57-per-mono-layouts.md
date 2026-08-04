# Stage 16.57 — Task 11 Phase 4b: Per-Mono Layouts

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.242.0 → v0.243.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.57 implements Task 11 Phase 4b — per-mono layouts keyed by
`(DefId, SubstsRef)`. Each generic type instantiation now gets its own
specialized `AdtLayout` with substituted field types.

**What was implemented**:

1. **`MonoLayoutKey`** — hashable key wrapping `(DefId, Vec<TyKind>)`. Uses
   `TyKind` (not `Ty`) because `TyKind` derives `Hash+Eq` while `Ty` is
   interned without these traits. Two keys are equal iff same DefId + same
   substs (element-wise TyKind comparison).

2. **`MonoLayoutMap`** — type alias for `HashMap<MonoLayoutKey, AdtLayout>`.
   Each entry is one specialized layout for a generic type instantiation.

3. **`build_mono_layouts(items, hir) -> MonoLayoutMap`** — builds per-mono
   layouts for all `MonoItem::Type` with non-empty substs. For each item:
   - Gets generic params via `generics_of`
   - Lowers each field type with `lower_hir_ty_to_mir_ty_with_generics`
     (resolves type params to `Param`)
   - Applies `substitute(field_ty, substs)` to replace `Param` with actual types
   - Builds an `AdtLayout` with the substituted field types

4. **Re-exports** in `src/mir/mod.rs`:
   - `pub use monomorphize::{build_mono_layouts, MonoLayoutKey, MonoLayoutMap, ...}`

5. **16 unit tests** covering:
   - MonoLayoutKey creation, equality, inequality, hashing (8 tests)
   - build_mono_layouts: empty, non-generic skip, generic struct, two
     instantiations, dedup, nested, correct field type, generic enum (8 tests)

**Key result**: `let b: Box<i32> = Box { val: 42 };` produces a MonoLayoutMap
with one entry: `MonoLayoutKey { Box, [i32] } → AdtLayout::Struct { field_tys: [i32] }`.
The field type is `i32` (substituted from `Param(T)`), not `Param` or `Error`.

**Test results**: 8059 tests passing (343 lib + 2492 integration + 5224
conformance), 0 failures, 0 warnings. +16 new unit tests.

## 2. Design Decisions

### 2.1 MonoLayoutKey Uses TyKind (通用 > 特解)

`Ty` is interned and doesn't implement `Hash+Eq`. `TyKind` derives both.
`MonoLayoutKey` stores `Vec<TyKind>` (extracted from substs) instead of
`Vec<Ty>`. This makes the key hashable without modifying the core `Ty` type.

Two keys with the same DefId and same TyKind substs are equal. This ensures
`Box<i32>` used in multiple places maps to one layout entry (dedup).

Per §1.0 原則 6 "通用 > 特例": one key type for all generic instantiations.

### 2.2 Non-Generic Types Skipped (高内聚低耦合)

`build_mono_layouts` only processes `MonoItem::Type` with non-empty substs.
Non-generic types (empty substs) continue to use the existing `AdtLayouts`
map (keyed by DefId only). This separation follows §16 — the per-mono map
is a new, separate data structure that doesn't interfere with existing
codegen.

### 2.3 Substituted Field Types (通解 > 特解)

For `struct Box<T> { val: T }` with substs `[i32]`:
1. `lower_hir_ty_to_mir_ty_with_generics(field.ty, generic_params)` →
   `Param(ParamTy { index: 0, name: T })`
2. `substitute(Param(0), [i32])` → `i32`
3. `AdtLayout::Struct { field_tys: [i32] }`

This is the full substitution pipeline from Phase 2, applied to layout
building. Per §1.0 原則 6 "通用 > 特例": one substitute function for all
type kinds, reused for layout building.

### 2.4 What This Does NOT Do (Phase 4c Boundary)

Stage 16.57 builds the per-mono layout map but does NOT integrate it into
codegen yet. The codegen still uses the existing `AdtLayouts` map (keyed
by DefId). Phase 4c will update codegen to:
- Check `MonoLayoutMap` first for generic types
- Fall back to `AdtLayouts` for non-generic types
- Use specialized names for LLVM type definitions

This separation allows incremental integration — the map is built and
tested independently before codegen consumes it.

## 3. Changes

### 3.1 New Types in `src/mir/monomorphize.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoLayoutKey {
    pub def_id: DefId,
    pub substs: Vec<TyKind>,
}

impl MonoLayoutKey {
    pub fn new(def_id: DefId, substs: &SubstsRef) -> Self
    pub fn from_mono_item(item: &MonoItem) -> Self
}

pub type MonoLayoutMap = HashMap<MonoLayoutKey, AdtLayout>;
```

### 3.2 New Function

```rust
pub fn build_mono_layouts(items: &[MonoItem], hir: &HirCrate) -> MonoLayoutMap
```

### 3.3 Re-exports in `src/mir/mod.rs`

```rust
pub use monomorphize::{
    build_mono_item_names, build_mono_layouts, collect_mono_items, mangle_ty,
    mangle_ty_with_interner, mono_item_name, MonoItem, MonoLayoutKey, MonoLayoutMap,
};
```

## 4. API (§23 Compliant)

| Type / Function | Pattern | Location |
|-----------------|---------|----------|
| `MonoLayoutKey` | `<Noun>_<Noun>_<Noun>` — data type | `src/mir/monomorphize.rs` |
| `MonoLayoutMap` | `<Noun>_<Noun>_<Noun>` — type alias | `src/mir/monomorphize.rs` |
| `build_mono_layouts` | `<verb>_<noun>_<noun>` — map builder | `src/mir/monomorphize.rs` |
| `MonoLayoutKey::new` | `<noun>` — constructor | `src/mir/monomorphize.rs` |
| `MonoLayoutKey::from_mono_item` | `<noun>_<prep>_<noun>` — constructor | `src/mir/monomorphize.rs` |

## 5. Test Plan

16 unit tests in `src/mir/monomorphize.rs`.

### Unit Tests (16)

| Category | Tests | Description |
|----------|-------|-------------|
| MonoLayoutKey | 8 | new, empty_substs, equality, inequality (def_id + substs), from_mono_item (Type + Fn), hashable |
| build_mono_layouts | 8 | empty, non-generic skip, generic struct, two instantiations, dedup, nested, correct field type, generic enum |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 343/343 PASS (+16 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2492/2492 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 8059 tests passing, 0 failures, 0 warnings.**

## 7. Version Policy

v0.242.0 → v0.243.0 (minor bump — new public API `MonoLayoutKey`,
`MonoLayoutMap`, `build_mono_layouts`. No existing API changes.)

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
| 4c | 🔧 Next | — | Codegen integration — use MonoLayoutMap in codegen |

## 9. References

- Stage 16.56 design: `docs/develop/v0/stage-16/stage-16.56-nested-generic-args-resolution.md`
- Stage 16.55 design: `docs/develop/v0/stage-16/stage-16.55-per-mono-codegen-naming.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Type system data flow: `docs/graph/type-system/data-flow.md`
- Stage Committee process: `docs/stage-committee-process.md` §13.4 + §23
