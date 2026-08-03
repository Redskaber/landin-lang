# Stage 15.73 — Type Propagation for Let Bindings + Move-of-Copy Fix

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.197.0 → v0.198.0
> **Process**: stage-committee-process.md v3.24 §29

## 1. Executive Summary

Stage 15.73 fixes two long-standing issues:

1. **Let binding type propagation**: When a `let` binding has no type annotation,
   the local's type is now taken from the init expression's type (if it's not
   Infer), instead of creating a fresh Infer type. This fixes struct/enum move
   errors where `let s2 = s` used `Operand::Copy` because s2's type was Infer
   (treated as Copy) at borrowck time.

2. **Move-of-Copy no-op**: The borrow checker now skips recording moves for
   Copy types. `Move` of a Copy type is semantically a copy (the source remains
   valid). This is needed because MIR lowerer uses `is_mir_ty_copy_conservative`
   (returns false for Adt) while borrow checker uses `is_copy` (returns true for
   Adt via unsound `ty_is_copy`). Without this, `let s2 = s` where s is a struct
   would mark s as moved, breaking 60+ tests.

**Key results**:
- `let s2 = s` where s is a struct now works correctly.
- 4 conformance tests flipped from compile_ok to compile_error (method-not-found
  errors now correctly caught because types are properly propagated).
- 1 lib test updated (use_after_move_detected now expects no errors for i32).
- All 7567 tests pass (221 lib + 2130 integration + 5216 conformance).

## 2. Root Cause

### 2.1 Let binding type propagation

At MIR lowering time, `let s = S { x: 1 }` creates:
- init_local (temp for struct literal): type = `Adt(DefId(0), [])`
- local_id (s): type = `Infer(TyVar(N))` (fresh, no annotation)

Then `let s2 = s` creates:
- init_local = s's local_id (returned by Path lowering)
- local_id (s2): type = `Infer(TyVar(M))` (fresh, no annotation)

At borrowck time, `is_mir_ty_copy_conservative(Infer)` returns `true` →
`Operand::Copy` is used → no move recorded → but typeck later resolves
s2's type to `Adt` (non-Copy) → the Copy was unsound.

**Fix**: Use the init expression's type directly when no annotation is present.

### 2.2 Move-of-Copy no-op

MIR lowerer uses `is_mir_ty_copy_conservative` (returns `false` for Adt) →
uses `Operand::Move` for struct assignments. Borrow checker uses `is_copy`
(returns `true` for Adt via unsound `ty_is_copy`) → considers structs as Copy.
Without the Move-of-Copy fix, `Move` always recorded a move, even for Copy types.

**Fix**: Skip recording moves for types that `is_copy` considers Copy.

## 3. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 221/221 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2130/2130 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS

**Total: 7567 tests passing, 0 failures, 0 warnings.**
