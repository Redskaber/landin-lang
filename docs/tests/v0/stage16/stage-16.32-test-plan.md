# Stage 16.32 — Test Plan: Triple-Nested Closure Typeck

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.230.3
> **Process**: stage-committee-process.md v3.24 §17.5 + §1.0 原則 6, 9

## 1. Test Scope

Stage 16.32 fixes TD-CLOSURE-TRIPLE-1 — the last remaining closure typeck
issue. Triple-nested closures (`|| || || x`) now compile AND run correctly.
The test plan verifies:

1. Triple-nested and quadruple-nested closures compile.
2. No regressions on existing closure patterns.
3. Runtime verification of triple-nested closures.

## 2. Test File

- `tests/v0/stage16/plan/stage16_32_triple_nested_closure_tests.rs`
- 12 tests, all passing ✅

## 3. Test Matrix

| # | Test Name | Verifies |
|---|-----------|----------|
| 1 | `stage16_32_triple_nested_compiles` | `|| || || x` compiles |
| 2 | `stage16_32_triple_nested_let_bindings` | Triple-nested with let bindings |
| 3 | `stage16_32_quadruple_nested_compiles` | `|| || || || x` compiles |
| 4 | `stage16_32_triple_nested_i32_capture` | Triple-nested with i32 capture |
| 5 | `stage16_32_triple_nested_with_param` | Triple-nested with innermost param |
| 6 | `stage16_32_double_nested_no_regression` | Double-nested still works |
| 7 | `stage16_32_nocapture_no_regression` | No-capture still works |
| 8 | `stage16_32_i32_capture_no_regression` | i32-capture still works |
| 9 | `stage16_32_mutable_capture_no_regression` | Mutable capture still works |
| 10 | `stage16_32_closure_returning_closure_with_param` | Closure returning closure |
| 11 | `stage16_32_multiple_closures` | Multiple closures in same function |
| 12 | `stage16_32_nested_multiple_captures` | Nested with multiple captures |

## 4. Runtime Verification

| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| `f()()()` where `f = \|\| \|\| \|\| x` | 42 | 42 | ✅ **NEW** |
| `f()()` (double-nested) | 42 | 42 | ✅ |
| `f() = 3` (mutable capture loop) | 3 | 3 | ✅ |
| `f(10)` (no-capture) | 11 | 11 | ✅ |

## 5. References

- Stage 16.32 design: `docs/develop/v0/stage-16/stage-16.32-triple-nested-closure-typeck.md`
- Stage 16.29-16.31 (prior closure fixes): `docs/develop/v0/stage-16/stage-16.29-*.md` etc.
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md`
