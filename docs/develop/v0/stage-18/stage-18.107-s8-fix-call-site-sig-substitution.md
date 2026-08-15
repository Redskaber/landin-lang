# Stage 18.107 — S8 Fix: Call-Site Sig Substitution

> **Author**: redskaber
> **Date**: 2026-08-11
> **Version**: v0.374.0 → v0.375.0
> **Status**: Active

## 1. 设计文档对齐（§13.1）

### 1.1 对应设计文档

- `docs/develop/v0/stage-18/stage-18.106-s7-fix-monoitem-skip-param.md` (S8 documented)
- `docs/develop/v0/task-11-monomorphization-design.md` §3.4 Phase 4

### 1.2 设计意图摘要

Stage 18.106 documented S8: call-site return type used generic sig (with
`Param(0)`) instead of substituted concrete type. This stage implements the
fix by extracting callee substs and applying `substitute(sig.output, substs)`.

## 2. Fix Implementation

### 2.1 Codegen Call Handler (src/codegen/terminator.rs)

1. Extract `callee_substs` from `FnDef(def_id, substs)` type (was discarded with `_`)
2. When resolving `call_ret_ty`: if `callee_substs` non-empty, apply
   `substitute(&sig.output, &callee_substs)` before converting to EmitType
3. Same substitution for `dest_ty` (store destination type)

### 2.2 MIR Lower Return Type (src/mir/lower/mod.rs)

1. `lower_hir_ty_to_mir_ty_with_lifetimes` now takes `generic_params` parameter
2. All recursive calls pass `generic_params` through
3. Fallback to `lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics` with `generic_params`
4. MIR lower param/return type lowering passes `&cx.generic_params`

### 2.3 Verification

Before fix:
- `make_box` generic MIR: `local 0: Adt(Box, [Error])` ❌
- Specialized `make_box<i32>`: `local 0: Adt(Box, [Error])` ❌
- Call site return type: `i32` (wrong for `make_box<bool>`)

After fix:
- `make_box` generic MIR: `local 0: Adt(Box, [Param(0)])` ✅ (S6 fix)
- Specialized `make_box<i32>`: `local 0: Adt(Box, [Int(I32)])` ✅
- Call site return type: `i1` for `id::<bool>` ✅ (was `i32`)

## 3. Remaining Issue (S9)

### S9: MonoItem::Type not collected for call-site concrete types

**Description**: `build_mono_layouts` produces 0 layouts because no `MonoItem::Type`
with concrete substs (e.g., `Box<i32>`) is collected. The call-site destination
locals still have `Adt(Box, [Param(0)])` (generic, from MIR lower), not
`Adt(Box, [i32])` (concrete, substituted).

**Reason**: MIR lower sets destination local type from `sig.output` (generic).
A writeback pass is needed to substitute destination locals with callee substs.

**Impact**: Specialized functions for generic types with nested Param return
(e.g., `make_box<bool>`) return `{ i32 }` instead of `{ i1 }` because the
mono layout lookup fails (no layout for `Box<bool>`).

**Fix plan**: v0.2 Phase 2 — add a writeback pass after typeck that substitutes
call-site destination local types with callee substs.

## 4. 验收（§3.2）

- [x] `cargo build --features llvm-backend` 成功
- [x] `cargo fmt --check` exit 0
- [x] `cargo clippy --all-targets --features llvm-backend -- -D warnings` 0 warnings
- [x] `cargo test --features llvm-backend --lib` 全绿 (640 passed)
- [x] `cargo test --features llvm-backend --tests` (skip runtime) 全绿 (2628 passed)
- [x] `id::<bool>` call returns `i1` ✅ (was `i32`)
- [x] Specialized `make_box<i32>` MIR: `local 0: Adt(Box, [Int(I32)])` ✅
- [x] S9 documented (destination local type writeback)

## 5. v0.2 Monomorphization Progress

| Phase | Status |
|-------|--------|
| Phase 1-4c (infrastructure) | ✅ Stage 16.52-16.59 |
| Turbofish FnDef substs | ✅ Stage 18.101 |
| Implicit inference FnDef substs | ✅ Stage 18.102 |
| Per-mono codegen (emit specialized fns) | ✅ Stage 18.103 |
| Call sites use specialized names | ✅ Stage 18.103 |
| S5: type_names pre-computed | ✅ Stage 18.104 |
| S6: nested Param return type | ✅ Stage 18.105 |
| S7: MonoItem collection skips Param/Error | ✅ Stage 18.106 |
| **S8: Call-site sig substitution** | ✅ Stage 18.107 |
| S9: Destination local type writeback | ❌ v0.2 Phase 2 |
| S2: method monomorphization | ❌ v0.2 Phase 2 |
