# Stage 18.111 — S9 Fix: Dest Local Type Writeback for Generic Calls

> **Author**: redskaber
> **Date**: 2026-08-15
> **Version**: v0.378.0 → v0.379.0
> **Status**: Active

## 1. Root Cause

After `writeback_fndef_substs` set the FnDef substs (e.g., `FnDef(make_box, [i32])`),
it skipped the Call terminator when substs were already populated (turbofish case).
The destination local's type remained `Adt(Box, [Param(0)])` instead of being
substituted to `Adt(Box, [i32])`.

## 2. Fix

In `writeback_fndef_substs`, restructured to:
1. If substs already exist (turbofish): use them directly (don't skip)
2. If substs are empty (implicit): infer from args (existing logic)
3. After getting substs (either way): `substitute(sig.output, substs)` and
   write back to the destination local's type

## 3. Verification

| Test | Before | After |
|------|--------|-------|
| `make_box::<bool>(true)` return type | `{ i32 }` ❌ | ✅ `{ i1 }` |
| `make_box::<i32>(42)` return type | `{ i32 }` ✅ | ✅ `{ i32 }` (no regression) |
| 640 lib tests | passed | ✅ 640 passed |
| 2663 integration tests | passed | ✅ 2663 passed, 0 skipped |
