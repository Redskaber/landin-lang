# Stage 18.101 — Turbofish Monomorphization (FnDef Substs Propagation)

> **Author**: redskaber
> **Date**: 2026-08-11
> **Version**: v0.368.0 → v0.369.0
> **Status**: Active

## 1. 设计文档对齐（§13.1）

### 1.1 对应设计文档

- `docs/develop/v0/task-11-monomorphization-design.md` §3.4 Phase 4 (Per-Mono Codegen)
- `docs/develop/v0/v0.1-capability-boundaries.md` v0.2 Roadmap P0 Monomorphization

### 1.2 设计意图摘要

Task 11 monomorphization infrastructure (Phases 1-4c) is complete, but a critical
gap remained: generic function calls with explicit turbofish (`id::<i32>(42)`)
did not propagate substs into the `FnDef` type in MIR. The `FnDef(def_id, Vec::new())`
creation in `mir/lower/expr_operand.rs` always used empty substs, so
`collect_mono_items` found 0 MonoItems for generic function calls.

### 1.3 已实现 / 偏差 / 未实现

| Item | Status |
|------|--------|
| Turbofish `id::<i32>(42)` → FnDef substs propagation | ✅ Stage 18.101 |
| `collect_mono_items` finds turbofish MonoItems | ✅ (verified, 2 items for id<i32>+id<bool>) |
| Implicit inference `id(42)` (no turbofish) → substs | ❌ TD-MONO-INFER (v0.2 type inference back-prop) |
| Per-mono codegen (emit specialized functions) | ❌ TD-MONO-CODEGEN (v0.2, requires substs propagation to codegen) |

## 2. 任务拆分（MUV）

| ID | Task | Acceptance |
|----|------|------------|
| 18.101.1 | Fix FnDef substs propagation in Path lowering | `lower_path_generic_args(path)` called at 2 FnDef creation sites |
| 18.101.2 | Add turbofish MonoItem test | `id::<i32>` + `id::<bool>` → 2 Fn MonoItems |
| 18.101.3 | Add non-generic no-MonoItem test | `add(1,2)` → 0 Fn MonoItems |
| 18.101.4 | Document TD-MONO-INFER (implicit inference gap) | Design doc + worklog |

## 3. Fix Details

### 3.1 Root Cause

`src/mir/lower/expr_operand.rs` Path lowering (lines 565-597) created `FnDef`
types with `Vec::new().into()` (empty substs):

```rust
// BEFORE (broken):
let fndef_ty = Ty::new(
    TyKind::FnDef(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
    expr.span,
);
```

This meant `collect_mono_items` (which checks `!substs.is_empty()`) found 0
MonoItems for generic function calls, even with explicit turbofish.

### 3.2 Fix

```rust
// AFTER (fixed):
let substs = lower_path_generic_args(path, &mut 0, cx.hir);
let fndef_ty = Ty::new(TyKind::FnDef(def_id, substs), expr.span);
```

`lower_path_generic_args` reads explicit turbofish args from the path (e.g.,
`::<i32>` → `[i32]`). For paths without turbofish, substs remain empty (this
is the TD-MONO-INFER gap — implicit type inference is v0.2 work).

### 3.3 Remaining Gap: TD-MONO-INFER

Implicit generic calls (`id(42)` without `::<i32>`) still produce empty substs
because MIR lowering happens before type inference back-propagates the concrete
type from the argument. Fixing this requires:

1. After typeck, walk all `FnDef` types in MIR local_decls
2. For each `FnDef(def_id, [])` with empty substs, look up the inferred
   substs from the typeck results (the unify table knows `T = i32` from
   unifying the arg type with the param type)
3. Write back the inferred substs into the FnDef type

This is a writeback-style pass, similar to `writeback_type_propagation` and
`writeback_closures`. Deferred to v0.2 as TD-MONO-INFER.

## 4. API Naming Compliance (§10)

- ✅ `lower_path_generic_args` already existed (§23 `<verb>_<noun>_<noun>` pattern)
- ✅ No new API — fix reuses existing function
- ✅ No glob re-exports

## 5. 验收（§3.2）

- [x] `cargo build --features llvm-backend` 成功
- [x] `cargo fmt --check` exit 0
- [x] `cargo clippy --all-targets --features llvm-backend -- -D warnings` 0 warnings
- [x] `cargo test --features llvm-backend --lib` 全绿 (640 passed)
- [x] `cargo test --features llvm-backend --tests` (skip runtime) 全绿 (2620 passed)
- [x] Turbofish test: `id::<i32>` + `id::<bool>` → 2 MonoItems ✅
- [x] Non-generic test: `add(1,2)` → 0 MonoItems ✅
- [x] No regression in existing tests
