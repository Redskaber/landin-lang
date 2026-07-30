# Stage 14.98 — Method Resolution on Let-Bound Locals

> **Author**: redskaber
> **Date**: 2026-07-30
> **Stage**: 14.98
> **Version**: v0.111.0 → v0.112.0

## Overview

Stage 14.98 fixes 4 P0 LLVM crashes on common code patterns where method calls
are made on let-bound locals whose init type isn't propagated by typeck. All
4 are fully fixed.

## LLVM Codegen Issues Fixed

### Bug Z1/Z2: Method call on struct in loop/match crashes

**Symptom**: `for i in 0..3 { let n = N { v: i }; sum += n.base(); }` crashed:
```
LLVM module verification failed: Called function must be a pointer!
  %v17 = call addrspace(32) i32 0({ i32 } %v16)
```

**Root cause**: `search_expr_for_local_init` only handled `Block` and `If` —
it didn't recurse into `While`/`For`/`Loop`/`Match` bodies. Method resolution
failed, emitting `Const{ty:Error, val:Int(0)}` → null function pointer.

**Fix**: Extended `search_expr_for_local_init` to handle all expression kinds.
Added `search_expr_for_local_init_expr` helper. For `Match`, look at the first
arm's body to determine the type.

### Bug Z3: Trait default body via intermediate `let` crashes

**Symptom**: `let r1 = p; let r2 = r1.g();` where `g` is a trait default body
crashed with LLVM null function pointer.

**Root cause**: `resolve_inherent_method_from_hir_expr`'s MethodCall-init
tracing arm only called `resolve_inherent_method`, not `resolve_trait_method`.

**Fix**: Added `.or_else(|| resolve_trait_method(...))` to 3 method-resolution
arms in `resolve_inherent_method_from_hir_expr`.

### Bug Z4: Method call on free function result crashes

**Symptom**: `let n = make_n(i); n.base();` (where `make_n` is a free function
returning a struct) crashed with LLVM null function pointer.

**Root cause**: `query_method_return_type` only searched `Impl` blocks, not
free `Fn` owners. Free-function return types couldn't be traced.

**Fix**: Extended `query_method_return_type` to search `HirItem::Fn` (free
functions) and `HirItem::Trait` (trait default bodies).

## Verification

All 4 fixes verified with end-to-end run_ok tests:
- `for i in 0..3 { let n = N{v:i}; sum += n.base(); }` → 3 ✅ (was: LLVM crash)
- `let n = match x { 0 => N{v:100}, _ => N{v:200} }; n.base()` → 200 ✅ (was: LLVM crash)
- `let r1 = p; let r2 = r1.g();` (trait default) → 8 ✅ (was: LLVM crash)
- `let n = make_n(i); n.base();` (free function) → 3 ✅ (was: LLVM crash)

## LLVM IR Pattern

The "Called function must be a pointer" error occurs when codegen emits:
```llvm
%v17 = call addrspace(32) i32 0({ i32 } %v16)
;                       ^^^ null function pointer
```

This happens when `method_def_id` is `None` and the lowering falls back to
`Const{ty:Error, val:Int(0)}`. The const's `Int(0)` value becomes the function
address, which LLVM rejects as a non-pointer.

The fix ensures method resolution succeeds in all common code patterns, so
`method_def_id` is always `Some(...)` when expected.
