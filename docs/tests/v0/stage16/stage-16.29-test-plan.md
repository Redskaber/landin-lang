# Stage 16.29 — Test Plan: Typeck on Synthesized Closure MIR Bodies

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.230.0
> **Process**: stage-committee-process.md v3.24 §17.5 + §1.0 原則 6, 9

## 1. Test Scope

Stage 16.29 closes the typeck gap via the 通解 (general solution): shared
unify table + typeck on each synthesized closure MIR body. The test plan
verifies:

1. ALL closures use the synthesized `call` function path (no special-case).
2. Nested closures compile (typeck + borrowck pass).
3. Closure Copy derivation (all-Copy captures → Copy).
4. Shared unify table (no TyVid collision / stack overflow).
5. Closure type accepted as callable in typeck.

## 2. Test File

- `tests/v0/stage16/plan/stage16_29_typeck_on_closure_mir_tests.rs`
- 15 tests, all passing ✅

## 3. Test Matrix

| # | Test Name | Verifies |
|---|-----------|----------|
| 1 | `stage16_29_nocapture_closure_synthesized` | No-capture → synthesized path |
| 2 | `stage16_29_i32_capture_synthesized` | i32-capture → synthesized path (通解) |
| 3 | `stage16_29_struct_capture_synthesized` | Struct-capture → synthesized path (was inline) |
| 4 | `stage16_29_nested_closure_compiles` | `|| || x` compiles (key test) |
| 5 | `stage16_29_nested_closure_let_binding` | `let g = f(); g()` compiles |
| 6 | `stage16_29_multiple_i32_captures` | Multiple i32 captures |
| 7 | `stage16_29_closure_with_while` | Closure with while loop |
| 8 | `stage16_29_closure_with_early_return` | Closure with early return |
| 9 | `stage16_29_closure_copy_derivation` | All-Copy captures → Copy |
| 10 | `stage16_29_closure_no_params` | `|| 42` (no params) |
| 11 | `stage16_29_closure_two_params` | `\|x, y\| x + y` (two params) |
| 12 | `stage16_29_chained_nocapture_calls` | `f(f(f(0)))` chained |
| 13 | `stage16_29_inline_path_deprecated` | Inline path not used |
| 14 | `stage16_29_shared_unify_table_no_overflow` | No stack overflow |
| 15 | `stage16_29_tuple_capture` | Tuple capture |

## 4. Coverage Analysis

### 4.1 Compile Coverage (typeck + borrowck)

- ✅ No-capture closures
- ✅ i32-capture closures
- ✅ Struct-capture closures (was inline before 16.29)
- ✅ Nested closures (`|| || x`)
- ✅ Multiple captures
- ✅ Closures with control flow (while, if/return)
- ✅ Closures with 0, 1, 2 params

### 4.2 Runtime Coverage (codegen)

- ✅ No-capture runtime: `f(10) = 11`
- ✅ i32-capture runtime: `x + y = 15`
- ❌ Nested closure runtime: codegen issue (TD-CLOSURE-CODEGEN-1)

### 4.3 Known Gaps (Tracked as TD)

- TD-CLOSURE-CODEGEN-1: Nested closure *runtime* (codegen for calling
  Closure-typed local). Compile passes, runtime fails.
- TD-CLOSURE-BORROWCK-1: Borrowck on closure MIR bodies (false positives
  on mutable captures in loops). Deferred — main body borrowck covers
  call sites.

## 5. Conformance Test Impact

- **Before Stage 16.29**: 5222/5224 passing (2 failures: nested closures)
- **After Stage 16.29**: 5224/5224 passing ✅

## 6. Integration with Existing Tests

- All Stage 16.13-16.28 tests still pass (no regressions).
- The `has_complex_captures` special-case routing is removed — all
  closures now go through the synthesized path.
- `lower_closure_call_inline` is `#[deprecated]` but retained for
  backward compatibility.

## 7. References

- Stage 16.29 design: `docs/develop/v0/stage-16/stage-16.29-typeck-on-synthesized-closure-mir.md`
- Task 10 design: `docs/develop/v0/task-10-closure-redesign-design.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md`
