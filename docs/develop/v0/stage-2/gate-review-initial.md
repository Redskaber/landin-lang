# Stage 2.x Gate Review Report

> **Date**: 2026-07-19
> **Reviewer**: Stage Committee (Explore agent, full audit)
> **Verdict**: ❌ **DO NOT ENTER Stage 3**
> **Action**: Schedule Stage 2.4 (typeck + borrowck correctness)

---

## Executive Summary

Stage 2.x (MIR + Type Check + Borrow Check) has **17 P0 blockers** that
would cause incorrect LLVM IR generation on any non-trivial program.

The root cause: each sub-stage (2.1/2.1b/2.2/2.2b/2.3) passed committee
review based on internal tests, but **no integration tests** verified that
the stages work together on real source code. The test suite is green
(541 tests pass) but never asks the type checker or borrow checker to
actually check anything on real programs.

---

## P0 Blockers (17 items)

### MIR Lowering (6 P0)
1. **14 expression kinds dropped to `TyKind::Error`** — Field, Index, Loop,
   While, For, Closure, MethodCall, Cast, AddrOf, Range, Array, Repeat,
   Struct, MacroCall, Unsafe, Try, Break, Continue
2. **All inference types share `TyVid(0)`** — every local/temp uses the
   same variable; type inference collapses
3. **Array lengths hardcoded `ConstVal::Uint(0)`** — codegen can't size arrays
4. **Projections never constructed** — Field/Index/Deref all fall to default
5. **Path resolution for `Res::Def` falls to error** — function calls have
   placeholder func operand
6. **Deref lowered as bitwise NOT** — `*p` produces `!p`

### Type Check (7 P0)
7. **`unify_resolved` missing 6 type kinds** — Adt, FnDef, FnPtr, Closure,
   Param, RawPtr all fall to mismatch error
8. **`bind_int_var_to_uint` hardcodes `i32`** — uint types silently corrupted
9. **Union-find doesn't propagate** — TyVar×TyVar merge is shallow
10. **`Terminator::Call` type checking discards everything** — `let _func_ty`
11. **`BinaryOp` discards RHS type** — `1 + true` accepted
12. **Resolved types not written back** — `mir.local_decls[i].ty` stays Infer
13. **`check_crate` never called** — no driver wires the pipeline

### Borrow Check (4 P0)
14. **Single-pass, no dataflow** — unsound on loops
15. **`place_path` collapses projections** — `a.x` == `a.y` (false positives)
16. **Borrows never expire** — "NLL" is actually lexical scope
17. **`Operand::Copy` doesn't check Copy-ness** — non-Copy types silently copied

---

## P1 Issues (8 items)
- Short-circuit And/Or not implemented (BitAnd/BitOr)
- String/byte literals mistyped as i32
- `HirTy.inferred` never populated
- `TraitResolver` not implemented
- Type/borrow errors not displayed to user
- Region inference deferred (all Region::Erased)
- No StorageLive/StorageDead in MIR
- No Assert terminator emitted

---

## Root Cause Analysis

The "小步快跑" approach worked well for Stages 0-1 (each sub-stage was
self-contained and testable in isolation). But Stage 2's sub-stages are
**not independently testable** — MIR lowering correctness depends on
type checking, which depends on borrow checking, which depends on the
driver wiring them together.

Each committee vote verified:
- ✅ The sub-stage's own code compiles
- ✅ The sub-stage's own tests pass
- ✅ fmt + clippy clean

But missed:
- ❌ Does the sub-stage work when fed real source code?
- ❌ Does the sub-stage produce output that the next stage can consume?
- ❌ Are the "P3 debt" items actually P0 in disguise?

---

## Remediation Plan: Stage 2.4

### Stage 2.4a — Core Type Infrastructure (3-5 days)
- Fix `TyVid(0)` sharing: allocate fresh TyVid per local/temp
- Write resolved types back to `mir.local_decls[i].ty`
- Populate `HirTy.inferred` with resolved types
- Add `src/driver.rs` with `compile()` entry point
- Wire `check_crate` + `borrowck::check_crate` into driver
- Add 10+ integration tests on real source

### Stage 2.4b — Lowering Completeness (3-5 days)
- Implement 14 missing HIR→MIR expression lowering kinds
- Add projection construction (Field/Index/Deref)
- Fix Deref (not bitwise NOT)
- Fix And/Or short-circuit (control flow, not BitOp)
- Add missing unify cases (Adt/FnDef/FnPtr/Closure/Param/RawPtr)
- Fix union-find (proper propagation)
- Fix `bind_int_var_to_uint`
- Add `Terminator::Call` type checking
- Add `BinaryOp` RHS type checking

### Stage 2.4c — Borrow Check Correctness (3-5 days)
- Implement field-sensitive `PlacePath` (LocalId + Vec<ProjectionElem>)
- Implement fixpoint dataflow for borrow checking
- Implement borrow expiry (last-use tracking, basic NLL)
- Check Copy-ness via typeck results
- Add 10+ borrowck integration tests on real source

### Stage 2.4d — Final Gate Review
- Re-run this full audit
- Require ≥30 integration tests on real source
- Require fibonacci + struct borrows + closures + loops to type-check
  and borrow-check with zero errors

**Estimated total: 2-3 weeks of focused work.**
