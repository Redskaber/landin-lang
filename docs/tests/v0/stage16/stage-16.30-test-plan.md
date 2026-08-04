# Stage 16.30 — Test Plan: Codegen for Closure-Typed Call Sites

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.230.1
> **Process**: stage-committee-process.md v3.24 §17.5 + §1.0 原則 5, 6, 9

## 1. Test Scope

Stage 16.30 fixes TD-CLOSURE-CODEGEN-1 — the codegen issue that prevented
nested closure runtime execution (`f()()` patterns). The test plan verifies:

1. Nested closures compile AND run correctly.
2. Let-bound closure call results work.
3. No regressions on existing closure patterns.
4. Dead code (Stage 4.13 inline path) is removed.

## 2. Test File

- `tests/v0/stage16/plan/stage16_30_closure_call_codegen_tests.rs`
- 12 tests, all passing ✅

## 3. Test Matrix

| # | Test Name | Verifies |
|---|-----------|----------|
| 1 | `stage16_30_nested_closure_compiles` | `f()()` compiles (key test) |
| 2 | `stage16_30_nested_closure_let_binding_compiles` | `let g = f(); g()` compiles |
| 3 | `stage16_30_nocapture_closure_no_regression` | No-capture still works |
| 4 | `stage16_30_i32_capture_no_regression` | i32-capture still works |
| 5 | `stage16_30_struct_capture_no_regression` | Struct-capture still works |
| 6 | `stage16_30_multiple_captures_no_regression` | Multiple captures still works |
| 7 | `stage16_30_two_params_no_regression` | Two-param closure still works |
| 8 | `stage16_30_chained_calls_no_regression` | Chained calls still work |
| 9 | `stage16_30_dead_code_removed` | Stage 4.13 path removed |
| 10 | `stage16_30_nested_closure_i32_capture` | Nested with i32 capture |
| 11 | `stage16_30_triple_nested_closure_deferred` | Triple-nested (TD-CLOSURE-TRIPLE-1) |
| 12 | `stage16_30_closure_returning_closure_with_param` | `|| |y| x + y` pattern |

## 4. Runtime Verification

| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| `f(10)` where `f = \|x\| x+1` | 11 | 11 | ✅ |
| `\|\| x + y` (i32 captures) | 15 | 15 | ✅ |
| `f()()` where `f = \|\| \|\| x` | 42 | 42 | ✅ **NEW** |
| `let g = f(); g()` (nested with let) | 1 | 1 | ✅ **NEW** |

## 5. Known Limitations (Tracked as TD)

- TD-CLOSURE-TRIPLE-1: Triple-nested closure (`|| || || x`) typeck.
  The middle closure's return type stays Infer, causing "expected
  function, found _". Needs deeper type inference. P3, follow-up.
- TD-CLOSURE-BORROWCK-1: Borrowck on closure MIR bodies (false positives
  on mutable captures in loops). P2, follow-up.

## 6. References

- Stage 16.30 design: `docs/develop/v0/stage-16/stage-16.30-closure-typed-call-codegen.md`
- Stage 16.29 (typeck gap fix): `docs/develop/v0/stage-16/stage-16.29-typeck-on-synthesized-closure-mir.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md`
