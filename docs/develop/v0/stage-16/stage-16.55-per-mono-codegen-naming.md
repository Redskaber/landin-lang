# Stage 16.55 — Task 11 Phase 4: Per-Mono Codegen — Specialized Naming

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.240.0 → v0.241.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.55 implements the foundation of Task 11 Phase 4 — the specialized
naming scheme for monomorphized items. This is the first step toward
per-mono codegen: each `MonoItem` gets a unique specialized name that can
be used as an LLVM symbol.

**What was implemented**:

1. **`mangle_ty(ty) -> String`** — mangles a `Ty` to a compact string
   without interner access (Adt uses DefId fallback). Examples:
   - `i32` → `"i32"`
   - `Adt(Box, [i32])` → `"Adt_5_i32"` (DefId fallback)
   - `Ref(_, _, i32)` → `"ref_i32"`
   - `Tuple([i32, bool])` → `"tuple_i32_bool"`

2. **`mangle_ty_with_interner(ty, type_name_by_def_id, interner) -> String`**
   — mangles a `Ty` with interner access for readable Adt names. Examples:
   - `Adt(Box, [i32])` → `"Box_i32"` (readable)
   - `Adt(Pair, [i32, bool])` → `"Pair_i32_bool"`

3. **`mono_item_name(item, base_name, type_name_by_def_id, interner) -> String`**
   — generates a specialized name for a MonoItem. Examples:
   - `Type { Box, [i32] }` + base "Box" → `"Box_i32"`
   - `Fn { id, [i32] }` + base "id" → `"id_i32"`
   - `Closure { def_id, [i32] }` + base "call" → `"call_i32"`

4. **`build_mono_item_names(items, fn_name_by_def_id, type_name_by_def_id, interner) -> HashMap<MonoItem, String>`**
   — builds a map from MonoItem to specialized name. For each item, looks
   up the base name from `fn_name_by_def_id` (for Fn), `type_name_by_def_id`
   (for Type), or generates `closure_<def_id>` (for Closure).

5. **Re-exports** in `src/mir/mod.rs`:
   - `pub use monomorphize::{build_mono_item_names, mangle_ty, mangle_ty_with_interner, mono_item_name, ...}`

6. **24 unit tests** covering all mangle_ty variants, mono_item_name, and
   build_mono_item_names.

**Key result**: `let b: Box<i32> = Box { val: 42 };` collects a MonoItem
`Type { Box, [i32] }`, which gets the specialized name `"Box_i32"`. This
name can be used as an LLVM symbol for the specialized type layout.

**Test results**: 8033 tests passing (327 lib + 2482 integration + 5224
conformance), 0 failures, 0 warnings. +24 new unit tests.

## 2. Design Decisions

### 2.1 Two Variants: With and Without Interner (通用 > 特例)

`mangle_ty` doesn't need an interner (uses DefId fallback for Adt).
`mangle_ty_with_interner` resolves Symbol to string for readable names.
This follows §1.0 原則 6 "通用 > 特例" — two variants for two use cases:
- `mangle_ty`: fast, no interner dependency (for internal use, debugging)
- `mangle_ty_with_interner`: readable (for codegen symbol names)

### 2.2 DefId Fallback (报错 > 静默)

When the type name can't be resolved (no interner, or DefId not in map),
`mangle_ty` uses `Adt_<def_id>` as fallback. This is always unique (DefId
is unique) and deterministic. Per §1.0 原則 5 "报错 > 静默": explicit
fallback rather than silent empty string or panic.

### 2.3 What This Does NOT Do (Phase 4 Boundary)

Stage 16.55 implements the **naming scheme** only. The actual codegen
integration (using these names in `emit_call`, layout generation, etc.)
is a follow-up stage because it requires:
- Changing `AdtLayouts` from `HashMap<DefId, AdtLayout>` to
  `HashMap<(DefId, SubstsRef), AdtLayout>`
- Emitting specialized function definitions for each `MonoItem::Fn`
- Updating `emit_call` to use specialized names

These are large changes that touch every codegen file. Stage 16.55
provides the foundation — the naming scheme — so the follow-up stages
can incrementally integrate it.

## 3. Changes

### 3.1 New Functions in `src/mir/monomorphize.rs`

```rust
pub fn mangle_ty(ty: &Ty) -> String
pub fn mangle_ty_with_interner(
    ty: &Ty,
    type_name_by_def_id: &HashMap<DefId, Symbol>,
    interner: &Rodeo,
) -> String
pub fn mono_item_name(
    item: &MonoItem,
    base_name: &str,
    type_name_by_def_id: &HashMap<DefId, Symbol>,
    interner: &Rodeo,
) -> String
pub fn build_mono_item_names(
    items: &[MonoItem],
    fn_name_by_def_id: &HashMap<DefId, String>,
    type_name_by_def_id: &HashMap<DefId, Symbol>,
    interner: &Rodeo,
) -> HashMap<MonoItem, String>
```

### 3.2 Re-exports in `src/mir/mod.rs`

```rust
pub use monomorphize::{
    build_mono_item_names, collect_mono_items, mangle_ty, mangle_ty_with_interner, mono_item_name,
    MonoItem,
};
```

## 4. API (§23 Compliant)

| Function | Pattern | Location |
|----------|---------|----------|
| `mangle_ty` | `<verb>_<noun>` — pure function | `src/mir/monomorphize.rs` |
| `mangle_ty_with_interner` | `<verb>_<noun>_<prep>_<noun>` — pure function | `src/mir/monomorphize.rs` |
| `mono_item_name` | `<noun>_<noun>_<noun>` — name generator | `src/mir/monomorphize.rs` |
| `build_mono_item_names` | `<verb>_<noun>_<noun>_<noun>` — map builder | `src/mir/monomorphize.rs` |

## 5. Test Plan

24 unit tests in `src/mir/monomorphize.rs`.

### Unit Tests (24)

| Category | Tests | Description |
|----------|-------|-------------|
| mangle_ty leaf | 5 | bool, i32, str, never, param |
| mangle_ty recursive | 7 | ref, refmut, tuple, empty tuple, array, slice, nested adt |
| mangle_ty generic | 4 | adt with substs, adt empty substs, fn_def, closure |
| mono_item_name | 5 | type with substs, fn with substs, empty substs, multiple substs, nested substs |
| build_mono_item_names | 3 | basic, empty, mixed (Fn + Type + Closure) |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 327/327 PASS (+24 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2482/2482 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 8033 tests passing, 0 failures, 0 warnings.**

## 7. Version Policy

v0.240.0 → v0.241.0 (minor bump — new public API `mangle_ty`,
`mangle_ty_with_interner`, `mono_item_name`, `build_mono_item_names`.
No existing API changes.)

## 8. Next Steps (Task 11 Roadmap)

| Phase | Status | Stage | Description |
|-------|--------|-------|-------------|
| 1a | ✅ | 16.50 | `generics_of` query |
| 1b | ✅ | 16.51 | Substs propagation into `TyKind::Adt` |
| 1c | ✅ | 16.52 | Substs propagation into `AggregateKind::Adt` |
| 2 | ✅ | 16.53 | `substitute(ty, substs)` function + integration |
| 3 | ✅ | 16.54 | `collect_mono_items` — walk MIR, dedup |
| 4a | ✅ | 16.55 | Specialized naming (`mangle_ty`, `mono_item_name`) |
| 4b | 🔧 Next | — | Codegen integration: layouts keyed by (DefId, SubstsRef) |
| 4c | 🔧 Planned | — | Emit specialized function definitions |

## 9. References

- Stage 16.54 design: `docs/develop/v0/stage-16/stage-16.54-monomorphization-collection.md`
- Stage 16.53 design: `docs/develop/v0/stage-16/stage-16.53-type-substitution.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Type system data flow: `docs/graph/type-system/data-flow.md`
- Stage Committee process: `docs/stage-committee-process.md` §13.4 + §23
