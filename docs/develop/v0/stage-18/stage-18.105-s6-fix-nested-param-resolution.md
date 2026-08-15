# Stage 18.105 — S6 Fix: Nested Param Return Type Resolution

> **Author**: redskaber
> **Date**: 2026-08-11
> **Version**: v0.372.0 → v0.373.0
> **Status**: Active

## 1. 设计文档对齐（§13.1）

### 1.1 对应设计文档

- `docs/develop/v0/stage-18/stage-18.104-mono-return-type-s6-investigation.md` (S6 root cause)
- `docs/develop/v0/task-11-monomorphization-design.md` §3.4 Phase 4

### 1.2 设计意图摘要

Stage 18.104 documented S6: generic function return types with nested Param
(e.g., `Box<T>` in `fn make_box<T>() -> Box<T>`) produced `Adt(Box, [Error])`
instead of `Adt(Box, [Param(0)])`. This stage implements the fix by passing
generics context through the type lowering chain.

## 2. Fix Implementation

### 2.1 Root Cause

`lower_ast_ty_to_mir_ty` (used by `lower_path_generic_args` to lower generic
args like `T` in `Box<T>`) could not resolve bare type parameters. It only
looked up struct/enum names by scanning HIR owners. Type parameters (`T`) are
not HIR owners.

### 2.2 Fix

1. **`lower_ast_ty_to_mir_ty_with_generics`**: New function that checks if a
   bare path name matches one of `generic_params`. If so, produces `Param(N)`.
2. **`lower_path_generic_args`**: Now takes `generic_params: &[ParamTy]` and
   passes it to `lower_ast_ty_to_mir_ty_with_generics`.
3. **`lower_hir_ty_to_mir_ty_with_hir_and_generics`**: New HIR-level variant
   that threads `generic_params` through the recursive type lowering.
4. **`MirLowerCtxt.generic_params`**: New field, set from HIR generics at
   MIR lowering start.
5. **Driver fn_sig_table**: Now uses `lower_hir_ty_to_mir_ty_with_hir_and_generics`
   with `generic_params` from `find_generics(def_id, hir)`.

### 2.3 Verification

Before fix: `fn_sigs[make_box].output = Adt(Box, [Error])` ❌
After fix: `fn_sigs[make_box].output = Adt(Box, [Param(0)])` ✅

## 3. Remaining Issue (S7)

### S7: MonoItem collection collects generic definition types

**Description**: `collect_mono_items` walks MIR bodies and collects
`MonoItem::Type` entries. For generic function bodies, it collects the
generic definition (e.g., `Box<T>` with `substs: [Param(0)]`) instead of
the concrete call-site instantiations (e.g., `Box<i32>`, `Box<bool>`).

**Reason**: `collect_from_ty` collects any `Adt(def_id, substs)` where
`!substs.is_empty()`. For `Adt(Box, [Param(0)])`, substs are non-empty
(contains Param), so it's collected — but this is the generic definition,
not a concrete instantiation.

**Impact**: `build_mono_layouts` produces layouts keyed by `[Param(0)]` and
`[Error]` instead of `[i32]` and `[bool]`. Specialized functions for
`make_box<bool>` return `{ i32 }` instead of `{ i1 }` because the layout
lookup fails (no layout for `[bool]`).

**Fix plan**: v0.2 Phase 2 — `collect_mono_items` should skip substs
containing `Param` (only collect fully-concrete instantiations). The
concrete instantiations come from call-site FnDef substs (already correct
from Stage 18.101/18.102).

## 4. API Naming Compliance (§10)

- ✅ `lower_ast_ty_to_mir_ty_with_generics` follows `<verb>_<noun>_<noun>_<prep>_<noun>`
- ✅ `lower_hir_ty_to_mir_ty_with_hir_and_generics` follows pattern
- ✅ `lower_path_generic_args` signature extended (backward-incompatible but internal)
- ✅ No glob re-exports

## 5. 验收（§3.2）

- [x] `cargo build --features llvm-backend` 成功
- [x] `cargo fmt --check` exit 0
- [x] `cargo clippy --all-targets --features llvm-backend -- -D warnings` 0 warnings
- [x] `cargo test --features llvm-backend --lib` 全绿 (640 passed)
- [x] `cargo test --features llvm-backend --tests` (skip runtime) 全绿 (2628 passed)
- [x] `fn_sigs[make_box].output = Adt(Box, [Param(0)])` ✅ (was [Error])
- [x] S7 documented (mono layout collection issue)

## 6. v0.2 Monomorphization Progress

| Phase | Status |
|-------|--------|
| Phase 1-4c (infrastructure) | ✅ Stage 16.52-16.59 |
| Turbofish FnDef substs | ✅ Stage 18.101 |
| Implicit inference FnDef substs | ✅ Stage 18.102 |
| Per-mono codegen (emit specialized fns) | ✅ Stage 18.103 |
| Call sites use specialized names | ✅ Stage 18.103 |
| S5: type_names pre-computed | ✅ Stage 18.104 |
| **S6: nested Param return type** | ✅ Stage 18.105 |
| S7: MonoItem collection skips Param substs | ❌ v0.2 Phase 2 |
| S2: method monomorphization | ❌ v0.2 Phase 2 |
