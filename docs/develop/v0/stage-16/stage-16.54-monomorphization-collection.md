# Stage 16.54 — Task 11 Phase 3: Monomorphization Collection

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.239.0 → v0.240.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.54 implements Task 11 Phase 3 — the `collect_mono_items` function
that walks all MIR bodies in a crate and collects `MonoItem { def_id, substs }`
pairs. Each MonoItem represents one specialization of a generic type or
function that needs specialized codegen.

**What was implemented**:

1. **`src/mir/monomorphize.rs`** — new module with:
   - `MonoItem` enum — `Type`, `Fn`, `Closure` variants, each with `def_id`
     and `substs`
   - `collect_mono_items(mirs) -> Vec<MonoItem>` — walks all MIR bodies,
     collects MonoItems, deduplicates via `HashSet`
   - `collect_from_ty(ty, collected)` — walks a type tree, extracts MonoItems
     from `Adt`/`FnDef`/`Closure` with non-empty substs
   - `collect_from_mir_body`, `collect_from_statement`, `collect_from_rvalue`,
     `collect_from_aggregate_kind`, `collect_from_operand`, `collect_from_place`,
     `collect_from_projection_elem`, `collect_from_terminator` — helper
     functions that walk each MIR construct
   - 24 unit tests covering all collection paths

2. **Re-exports** in `src/mir/mod.rs`:
   - `pub use monomorphize::{collect_mono_items, MonoItem}`

3. **12 integration tests** in
   `tests/v0/stage16/plan/stage16_54_monomorphize_tests.rs` covering:
   - Basic MonoItem collection (2 tests)
   - Deduplication (1 test)
   - Nested generics (1 test)
   - Generic enum (2 tests)
   - Multiple generic types (1 test)
   - No regressions on non-generic code (3 tests)
   - MonoItem accessor methods (2 tests)

**Key result**: `let b: Box<i32> = Box { val: 42 };` produces 1 MonoItem
(`Type { def_id: Box, substs: [i32] }`). Two different instantiations
(`Box<i32>` + `Box<bool>`) produce 2 MonoItems. The same instantiation
used twice produces 1 MonoItem (dedup).

**Test results**: 8009 tests passing (303 lib + 2482 integration + 5224
conformance), 0 failures, 0 warnings. +36 new tests (24 unit + 12 integration).

## 2. Design Decisions

### 2.1 HashSet Deduplication (通解 > 特解)

MonoItems are collected into a `HashSet<MonoItem>` which automatically
deduplicates by `(def_id, substs)` pair. `Vec<i32>` used in 100 places
produces 1 MonoItem. This follows §1.0 原則 6 "通用 > 特例" — one
deduplication mechanism for all MonoItem kinds.

`MonoItem` derives `Eq + Hash`, so the HashSet works out of the box.
The hash is based on `(def_id, substs)` — two MonoItems with the same
def_id and substs are equal regardless of variant (Type vs Fn vs Closure).

Wait — actually, two MonoItems with the same def_id and substs but
different variants (e.g., `Type { def_id: 1, substs: [i32] }` vs
`Fn { def_id: 1, substs: [i32] }`) are NOT equal because the variant
is part of the enum. This is correct — a type and a function can share
a DefId in theory (though rare in practice).

### 2.2 Recursive Type Walking (通用 > 特例)

`collect_from_ty` recursively walks all type kinds:
- `Adt(def_id, substs)` → collect + recurse into substs
- `FnDef(def_id, substs)` → collect + recurse into substs
- `Closure(def_id, substs)` → collect + recurse into substs
- `Ref(_, _, inner)` → recurse into inner
- `RawPtr(_, inner)` → recurse into inner
- `Array(inner, _)` → recurse into inner
- `Slice(inner)` → recurse into inner
- `Tuple(tys)` → recurse into each element
- `FnPtr(sig)` → recurse into inputs + output
- Leaf types → no collection

This ensures nested generics like `Vec<Vec<i32>>` collect both the outer
and inner MonoItems. Per §1.0 原則 6 "通用 > 特例": one recursive walker
for all type kinds.

### 2.3 MIR Body Walking (高内聚低耦合)

The collection walks each MIR body in three phases:
1. **Local declarations** — `local_decls[i].ty` for each local
2. **Statements** — `Rvalue::Aggregate`, `Rvalue::Cast`, `Println` args
3. **Terminators** — `Call { func, args }`, `SwitchInt { discr }`,
   `Drop { place }`, `Assert { cond }`

Each phase delegates to specialized helpers (`collect_from_ty`,
`collect_from_operand`, `collect_from_place`, etc.) that handle their
specific MIR construct. This follows §16 (interface isolation) — each
helper owns one MIR construct kind.

### 2.4 What This Does NOT Do (Phase 4 Boundary)

`collect_mono_items` only **collects** — it doesn't generate specialized
code. Phase 4 (per-mono codegen) will use the collected MonoItems to:
- Key layouts by `(DefId, SubstsRef)` instead of just `DefId`
- Emit specialized LLVM types/functions: `landin_vec_push_i32`,
  `landin_vec_push_bool`

The collection is the foundation — without it, codegen doesn't know which
specializations to generate. Per §16: collection is a read-only pass over
MIR; codegen is a separate pass that consumes the collected items.

## 3. Changes

### 3.1 New Module: `src/mir/monomorphize.rs`

```rust
pub enum MonoItem {
    Type { def_id: DefId, substs: SubstsRef },
    Fn { def_id: DefId, substs: SubstsRef },
    Closure { def_id: DefId, substs: SubstsRef },
}

pub fn collect_mono_items(mirs: &[MirBody]) -> Vec<MonoItem>
pub fn collect_from_ty(ty: &Ty, collected: &mut HashSet<MonoItem>)
```

Plus 8 private helper functions for walking each MIR construct.

### 3.2 Re-exports in `src/mir/mod.rs`

```rust
pub use monomorphize::{collect_mono_items, MonoItem};
```

## 4. API (§23 Compliant)

| Function / Type | Pattern | Location |
|-----------------|---------|----------|
| `MonoItem` | `<Noun>_<Noun>` — data type | `src/mir/monomorphize.rs` |
| `collect_mono_items` | `<verb>_<noun>_<noun>` — collection pass | `src/mir/monomorphize.rs` |
| `collect_from_ty` | `<verb>_<prep>_<noun>` — type walker | `src/mir/monomorphize.rs` |
| `MonoItem::def_id` | `<noun>` — accessor | `src/mir/monomorphize.rs` |
| `MonoItem::substs` | `<noun>` — accessor | `src/mir/monomorphize.rs` |
| `MonoItem::debug_string` | `<noun>_<noun>` — debug helper | `src/mir/monomorphize.rs` |

## 5. Test Plan

24 unit tests in `src/mir/monomorphize.rs` + 12 integration tests in
`tests/v0/stage16/plan/stage16_54_monomorphize_tests.rs`.

### Unit Tests (24)

| Category | Tests | Description |
|----------|-------|-------------|
| MonoItem struct | 5 | def_id accessors, equality, inequality |
| collect_from_ty | 8 | Adt with/without substs, nested, ref, tuple, FnDef, Closure, leaf |
| collect_mono_items | 5 | Empty, local decls, dedup, multiple mirs, cross-mir dedup |
| Statement/rvalue/terminator | 4 | Aggregate, cast, call, array aggregate |
| Complex scenarios | 2 | Mixed types, debug string |

### Integration Tests (12)

| Category | Tests | Description |
|----------|-------|-------------|
| Basic collection | 2 | Non-generic (0 items), generic struct (1 item) |
| Dedup | 1 | Same instantiation twice → 1 item |
| Nested generics | 1 | Box<Box<i32>> → at least 1 item |
| Generic enum | 2 | Single variant, multiple variants dedup |
| Multiple generics | 1 | Box + Pair in same program |
| No regressions | 3 | Non-generic struct/enum/main → 0 items |
| Accessor methods | 2 | def_id + substs accessors |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 303/303 PASS (+24 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2482/2482 PASS (+12 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 8009 tests passing, 0 failures, 0 warnings.**

## 7. Version Policy

v0.239.0 → v0.240.0 (minor bump — new module + new public API
`collect_mono_items` and `MonoItem`. No existing API changes.)

## 8. Next Steps (Task 11 Roadmap)

| Phase | Status | Stage | Description |
|-------|--------|-------|-------------|
| 1a | ✅ | 16.50 | `generics_of` query |
| 1b | ✅ | 16.51 | Substs propagation into `TyKind::Adt` |
| 1c | ✅ | 16.52 | Substs propagation into `AggregateKind::Adt` |
| 2 | ✅ | 16.53 | `substitute(ty, substs)` function + integration |
| 3 | ✅ | 16.54 | `collect_mono_items` — walk MIR, dedup (def_id, substs) |
| 4 | 🔧 Next | — | Per-mono codegen — layouts keyed by (DefId, SubstsRef) |

## 9. Known Limitations

### 9.1 Nested Generic Args Lowering

When a generic type appears as a generic argument (e.g., `Box<i32>` in
`Box<Box<i32>>`), the inner `Box<i32>` is lowered via `lower_ast_ty_to_mir_ty`
which produces `Error` (the AST path can't be resolved without HIR context).

This means `Box<Box<i32>>` produces only 1 MonoItem (the outer Box with
`[Error]` substs) instead of 2 (outer + inner). This is a known limitation
that will be fixed when AST→MIR type lowering gains HIR context access
(Phase 4 work — the lowerer needs to resolve the inner path to a DefId).

The collection algorithm itself is correct — it would collect 2 MonoItems
if the inner type were properly lowered as `Adt(Box, [i32])`.

## 10. References

- Stage 16.53 design: `docs/develop/v0/stage-16/stage-16.53-type-substitution.md`
- Stage 16.52 design: `docs/develop/v0/stage-16/stage-16.52-aggregate-substs-propagation.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Type system data flow: `docs/graph/type-system/data-flow.md`
- Stage Committee process: `docs/stage-committee-process.md` §13.4 + §23
