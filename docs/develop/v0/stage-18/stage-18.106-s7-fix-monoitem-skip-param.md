# Stage 18.106 — S7 Fix: MonoItem Collection Skips Param/Error Substs

> **Author**: redskaber
> **Date**: 2026-08-11
> **Version**: v0.373.0 → v0.374.0
> **Status**: Active

## 1. 设计文档对齐（§13.1）

### 1.1 对应设计文档

- `docs/develop/v0/stage-18/stage-18.105-s6-fix-nested-param-resolution.md` (S7 documented)
- `docs/develop/v0/task-11-monomorphization-design.md` §3.4 Phase 4

### 1.2 设计意图摘要

Stage 18.105 documented S7: `collect_mono_items` collected generic definition
types (e.g., `Box<T>` with `[Param(0)]`) instead of only concrete instantiations.
This stage implements the fix by skipping substs containing `Param` or `Error`.

## 2. Fix Implementation

### 2.1 Root Cause

`collect_from_ty` collected any `Adt(def_id, substs)` / `FnDef(def_id, substs)` /
`Closure(def_id, substs)` where `!substs.is_empty()`. For generic function bodies,
this collected the generic definition (e.g., `Adt(Box, [Param(0)])`) — not a
concrete instantiation.

### 2.2 Fix

Added `substs_are_concrete(substs)` check: returns `true` only if no subst
contains `Param` or `Error` (recursively). Applied to all three generic-capable
types (Adt, FnDef, Closure).

```rust
// Before: collect if substs non-empty
if !substs.is_empty() { collected.insert(...); }

// After: collect if substs non-empty AND concrete
if !substs.is_empty() && substs_are_concrete(substs) { collected.insert(...); }
```

### 2.3 Verification

Before fix: `MonoItems = [Fn{i32}, Fn{bool}, Type{Box, [Param(0)]}, Type{Box, [Error]}]` ❌
After fix: `MonoItems = [Fn{i32}, Fn{bool}]` ✅ (only concrete instantiations)

## 3. Remaining Issue (S8)

### S8: Call-site return type uses generic sig (not substituted)

**Description**: When codegen resolves a generic function call's return type,
it reads `fn_sigs[def_id].output` which contains `Param(0)` (the generic sig).
The call-site-specific substs (e.g., `[i32]`) are not applied to the sig output.

**Reason**: `fn_sig_table` stores one sig per DefId (the generic sig). Call sites
need to substitute the sig with the call's substs to get the concrete return type.

**Impact**: `make_box::<i32>(42)` call site sees return type `Adt(Box, [Param(0)])`
instead of `Adt(Box, [i32])`. The destination local gets the wrong type, and
no `MonoItem::Type { Box, [i32] }` is collected (because the type contains Param).

**Fix plan**: v0.2 Phase 2 — in codegen Call handling, apply `substitute(sig.output, substs)`
before using the return type. This requires the call-site substs (available from
the FnDef type).

## 4. API Naming Compliance (§10)

- ✅ `substs_are_concrete` follows `<noun>_<verb>_<adj>` pattern
- ✅ `type_contains_param_or_error` follows `<noun>_<verb>_<noun>_<prep>_<noun>` pattern
- ✅ No new public API (internal helpers)
- ✅ No glob re-exports

## 5. 验收（§3.2）

- [x] `cargo build --features llvm-backend` 成功
- [x] `cargo fmt --check` exit 0
- [x] `cargo clippy --all-targets --features llvm-backend -- -D warnings` 0 warnings
- [x] `cargo test --features llvm-backend --lib` 全绿 (640 passed)
- [x] `cargo test --features llvm-backend --tests` (skip runtime) 全绿 (2628 passed)
- [x] MonoItems no longer contain Param/Error substs ✅
- [x] S8 documented (call-site sig substitution)

## 6. v0.2 Monomorphization Progress

| Phase | Status |
|-------|--------|
| Phase 1-4c (infrastructure) | ✅ Stage 16.52-16.59 |
| Turbofish FnDef substs | ✅ Stage 18.101 |
| Implicit inference FnDef substs | ✅ Stage 18.102 |
| Per-mono codegen (emit specialized fns) | ✅ Stage 18.103 |
| Call sites use specialized names | ✅ Stage 18.103 |
| S5: type_names pre-computed | ✅ Stage 18.104 |
| S6: nested Param return type | ✅ Stage 18.105 |
| **S7: MonoItem collection skips Param/Error** | ✅ Stage 18.106 |
| S8: Call-site sig substitution | ❌ v0.2 Phase 2 |
| S2: method monomorphization | ❌ v0.2 Phase 2 |
