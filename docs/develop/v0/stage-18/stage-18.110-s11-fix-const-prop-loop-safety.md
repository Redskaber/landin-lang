# Stage 18.110 — S11 Fix: Const-Prop Loop Safety

> **Author**: redskaber
> **Date**: 2026-08-15
> **Version**: v0.377.0 → v0.378.0
> **Status**: Active

## 1. Root Cause

`run_const_prop` processes basic blocks in order (bb0→bb1→bb2→...). When it
reaches bb1 (loop condition), `i = 0` is in `const_map`. It folds `0 < 3`
→ `true`. But `i` is modified in the loop body (bb4: `i = i + 1`), which
hasn't been processed yet. The folded `true` becomes the permanent loop
condition, causing an infinite loop.

## 2. Fix

1. **Detect back-edges**: Scan all basic blocks for `Goto(target)` where
   `target <= current` (loop back-edge).
2. **Clear const_map at loop headers**: When entering a BB that is the target
   of a back-edge, clear `const_map` — loop variables may have been modified.
3. **Skip BinaryOp folding when back-edges exist**: When loops are present,
   don't fold BinaryOp results (only propagate constants into operands).

## 3. Verification

| Test | Before | After |
|------|--------|-------|
| `while i < 3` | infinite loop ❌ | ✅ 3× "hello" |
| `loop { break; }` | infinite loop ❌ | ✅ correct break |
| `while { continue; }` | infinite loop ❌ | ✅ correct continue |
| rt_div / rt_mod | panic ❌ | ✅ (fixed in S10) |
| 640 lib tests | passed | ✅ 640 passed |
| 2663 integration tests | 4 skipped (S11) | ✅ **2663 passed, 0 skipped!** |

## 4. All Runtime Tests Now Pass

For the first time, ALL 35 runtime tests pass (rt_add, rt_div, rt_mod,
rt_break, rt_continue, rt_loop_break, rt_while, etc.) — no more OOM skips
or infinite loop hangs.
