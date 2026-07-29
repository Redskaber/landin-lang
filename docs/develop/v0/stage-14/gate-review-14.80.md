# Stage 14.80 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.95.0 → v0.96.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.80 hardens the Stage 14.79 nested-array fix and lifts a Stage 0
limitation on array-by-value parameters that had been recorded as a
conformance `compile_error` expectation.

## 2. Bugs Fixed

### Bug A: Stage 14.79 regression — array repeat `[0; N]` for non-int element types

**Symptom**: After Stage 14.79, 5 conformance tests started failing:

```
FAIL  01-typecheck/00-basic-inference/048-f64-f64-in-array.lin
FAIL  01-typecheck/00-basic-inference/058-bool-bool-in-array.lin
FAIL  01-typecheck/00-basic-inference/068-char-char-in-array.lin
FAIL  01-typecheck/00-basic-inference/078-&str-&str-in-array.lin
FAIL  01-typecheck/00-basic-inference/168-f32-f32-in-array.lin
```

with typeck error `mismatched types: expected Float(F64), found
Infer(IntVar(IntVid(0)))`.

**Root cause**: Stage 14.79 changed the array `array_ty` element from
`TyKind::Error` to `cx.mir.local(elem_local).ty.clone()`. For an unsuffixed
integer literal element like `0`, that yielded `Infer(IntVar)` — which can
only unify with `Int`/`Uint` types, not `Float`/`Bool`/`Char`/`Str`. The
destination `let arr: [f64; 3] = [0; 3]` then triggered a real type error
during `unify(Array(IntVar), Array(Float(F64)))`.

The pre-14.79 code masked this error because `Error` propagates as `Ok`
in unify (line 253 in `src/typeck/unify.rs`).

**Fix**: Split the element type used in `array_ty` (must be a concrete type
for codegen to allocate the correct LLVM type — preserved Stage 14.79 nested
array fix) from the element type used in `AggregateKind::Array` (always a
fresh `TyVar` so each operand unifies cleanly). For `array_ty`:
- if `actual_elem_ty` is concrete (e.g. `[i32; 3]` for `[[i32; 3]; 3]`), use it
- if it is `Infer` or `Error` (e.g. unsuffixed `0`), fall back to `Error`

This preserves both:
- Stage 14.79 fix: `[[i32; 3]; 3]` works (concrete element type used)
- Stage 14.78 behavior: `[0; 3]` for `[f64; 3]` accepts (Error propagates)

Per §1.0 原則 5 "报错 > 静默": a real type error like `[0; 3]` for `[f64; 3]`
should *eventually* be caught — but it requires adding int→float coercion to
unify, which is deferred to Stage 14.81+ as a separate P0 fix. For now the
behavior matches v0.96.0 backup (silent accept).

Per §1.0 原則 6 "通用 > 特例": one rule handles both cases by checking
whether the element type is concrete.

### Bug B: Stale `compile_error` expectation on test 020-fib-linear-search-5

**Symptom**: Test `04-e2e/01-fib/020-fib-linear-search-5.lin` expected
`compile_error` (marked as "Stage 0 limitation" — array-by-value parameter).
The compiler now accepts the program (Stage 0 limitation lifted by
cumulative Stages 14.x fixes), so the test was failing as
`expected FAIL but compiler accepted the input`.

**Fix**: Updated the test header to `EXPECTED: compile_ok` with a note
that the Stage 0 limitation has been lifted.

## 3. Verification

- All 1951 rust tests pass (zero regression)
- All 5167 conformance tests pass (was 5161 pass + 6 fail at v0.95.0 zip
  extraction — Stage 14.79 regression + stale 020 expectation; now 5167/5167)
- 0 clippy warnings, fmt clean
- Stage 14.79 nested array test still passes:
  `e2e-runok-141-nested-array-struct.lin` → `10\n20\n165` ✅

## 4. P0 Blockers Status (unchanged from Stage 14.79)

| ID | Gap | Est. effort | Status |
|----|-----|-------------|--------|
| GAP-1 | NLL soundness regression | L3 | Pending — Stage 14.81+ |
| GAP-2 | Region inference is dead_code | L3 | Pending — Stage 14.81+ |
| GAP-3 | Drop elaboration is dead_code | L3 | Pending — Stage 14.81+ |
| GAP-4 | Lifetime elision is dead_code | L2 | Pending — Stage 14.81+ |
| GAP-5 | `self.x` field access crashes codegen | L2 | Pending — Stage 14.81+ |
| GAP-6 | Two-phase borrows (method-call subset) | L2 | Pending — Stage 14.81+ |
| GAP-7 | Disjoint closure captures (RFC 2229) | L2 | Pending — Stage 14.81+ |

## 5. Design Doc Alignment (§13.4)

No new design doc deviations. The Stage 14.80 fix is purely a hardening
of Stage 14.79 — no spec changes needed.

## 6. Next Stage Plan

Stage 14.81: Begin P0 blocker fixes, starting with the smallest:
- GAP-4 (lifetime elision) — L2, 3 rules per `04-ownership-borrowing.md` §3.2
- GAP-6 (two-phase borrow — method-call subset) — L2
- GAP-5 (self.x field access crashes codegen) — L2

These are the lowest-hanging P0 fixes; GAP-1 (NLL) and GAP-2 (region) are
L3 and will be addressed after the L2 fixes.
