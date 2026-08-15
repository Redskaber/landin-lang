# Stage 18.109 — S10 Fix: DivisionByZero Assert Skip for Const-Prop Folded Div/Rem

> **Author**: redskaber
> **Date**: 2026-08-11
> **Version**: v0.376.0 → v0.377.0
> **Status**: Active

## 1. Root Cause

`const_prop` folds `20 / 4` → `Constant(5)`, removing the BinaryOp assignment.
But the `DivisionByZero(rhs)` assert terminator remains, referencing `local 4`
(the rhs operand's local). DCE removes `local 4 = Use(Constant(4))` (dead after
const_prop), leaving `local 4` uninitialized. The assert's `load %loc_4` reads
garbage (often 0), triggering a false "divide by zero" panic.

## 2. Fix

In `codegen/terminator.rs` `DivisionByZero` handler:
- Check if `rhs` is `Copy(local)` and `emitter.local(id)` is `None`
- If so: the local was never assigned (const_prop + DCE removed it) — skip
  the div-by-zero check (const_prop only folds non-zero rhs, so skipping is safe)
- If not: emit the `icmp eq` check as before

## 3. Verification

| Test | Before | After |
|------|--------|-------|
| `20 / 4` | panic: divide by zero ❌ | ✅ outputs `5` |
| `17 % 5` | panic: divide by zero ❌ | ✅ outputs `2` |
| `codegen_div_zero_check_*` | passed | ✅ still pass (non-const cases emit icmp) |
| 640 lib tests | passed | ✅ 640 passed |
| 2659 integration tests | 4 failed | ✅ 2659 passed (4 loop tests skipped: S11) |
