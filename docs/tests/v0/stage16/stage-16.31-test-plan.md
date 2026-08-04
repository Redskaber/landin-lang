# Stage 16.31 — Test Plan: Borrowck on Closure MIR Bodies

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.230.2
> **Process**: stage-committee-process.md v3.24 §17.5 + §1.0 原則 4, 9

## 1. Test Scope

Stage 16.31 fixes TD-CLOSURE-BORROWCK-1 — the soundness gap where
borrowck was silently skipped on closure MIR bodies. The test plan verifies:

1. Mutable captures in loops/conditionals work.
2. Early return inside closures works.
3. Borrowck violations inside closures are detected (soundness).
4. No regressions on existing closure patterns.

## 2. Test File

- `tests/v0/stage16/plan/stage16_31_borrowck_on_closure_mir_tests.rs`
- 14 tests, all passing ✅

## 3. Test Matrix

| # | Test Name | Verifies |
|---|-----------|----------|
| 1 | `stage16_31_mutable_capture_while_loop` | `|| { while x<3 { x+=1; } x }` compiles |
| 2 | `stage16_31_early_return_in_closure` | `|| { if x>0 { return 1; } 0 }` compiles |
| 3 | `stage16_31_mutable_capture_compound_assign` | `|| { x += 5; }` compiles |
| 4 | `stage16_31_multiple_mutable_captures` | `|| { a += b; b += 1; }` compiles |
| 5 | `stage16_31_mixed_captures` | Mix of mut and immut captures |
| 6 | `stage16_31_use_after_move_in_closure_detected` | Soundness: use-after-move detected |
| 7 | `stage16_31_nocapture_no_regression` | No-capture still works |
| 8 | `stage16_31_i32_capture_no_regression` | i32-capture still works |
| 9 | `stage16_31_nested_closure_no_regression` | Nested closure still works |
| 10 | `stage16_31_mutable_capture_if_else` | Mut capture with if-else |
| 11 | `stage16_31_mutable_capture_loop` | Mut capture with loop |
| 12 | `stage16_31_mutable_capture_for_loop` | Mut capture with for-loop (Range) |
| 13 | `stage16_31_capture_mutability_propagation` | Capture mutability propagated |
| 14 | `stage16_31_closure_returning_mutable_capture` | Closure returning mut capture |

## 4. Runtime Verification

| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| `f()` where `f = \|\| { while x<3 { x+=1; } x }` | 3 | 3 | ✅ **NEW** |
| `f()` where `f = \|\| { if x>0 { return 1; } 0 }` | 1 | 1 | ✅ |
| `f(10)` (no-capture) | 11 | 11 | ✅ |
| `x + y` (i32 capture) | 15 | 15 | ✅ |
| `f()()` (nested) | 42 | 42 | ✅ |

## 5. Soundness Verification

- ✅ Use-after-move inside closures is now detected (test 6)
- ✅ Borrowck violations inside closure bodies are reported
- ✅ No false positives on mutable captures (tests 1-5, 10-14)

## 6. Known Limitations (Tracked as TD)

- TD-CLOSURE-TRIPLE-1: Triple-nested closure (`|| || || x`) typeck.
  P3, follow-up.

## 7. References

- Stage 16.31 design: `docs/develop/v0/stage-16/stage-16.31-borrowck-on-closure-mir.md`
- Stage 16.29 (typeck gap fix): `docs/develop/v0/stage-16/stage-16.29-typeck-on-synthesized-closure-mir.md`
- Stage 16.30 (codegen fix): `docs/develop/v0/stage-16/stage-16.30-closure-typed-call-codegen.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md`
