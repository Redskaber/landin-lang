# Stage 16.34 — Test Plan: Clean Up Inline Closure Path

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.231.0
> **Process**: stage-committee-process.md v3.24 §17.5 + §1.0 原則 5

## 1. Test Scope

Stage 16.34 completes Task 10 Step 5 — the final cleanup of the closure
redesign. The test plan verifies that removing the `closure_bodies`
side-table and `lower_closure_call_inline` doesn't break any closure patterns:

1. All closure patterns still compile (type-based check works)
2. Let-bound closures work (type propagation works)
3. Re-let-bound closures work (`let h = g;` where g is a closure)
4. No regressions on any closure feature

## 2. Test File

- `tests/v0/stage16/plan/stage16_34_cleanup_inline_path_tests.rs`
- 12 tests, all passing ✅

## 3. Test Matrix

| # | Test Name | Verifies |
|---|-----------|----------|
| 1 | `stage16_34_nocapture_closure_type_check` | No-capture (type-based check) |
| 2 | `stage16_34_i32_capture_type_check` | i32-capture (type-based check) |
| 3 | `stage16_34_let_bound_closure` | Let-bound closure (`let g = f;`) |
| 4 | `stage16_34_re_let_bound_closure` | Re-let-bound (`let h = g;`) |
| 5 | `stage16_34_struct_capture_type_check` | Struct-capture |
| 6 | `stage16_34_mutable_capture_type_check` | Mutable capture |
| 7 | `stage16_34_nested_closure_type_check` | Double-nested |
| 8 | `stage16_34_triple_nested_type_check` | Triple-nested |
| 9 | `stage16_34_let_bound_nested_closure` | Let-bound nested |
| 10 | `stage16_34_two_params_type_check` | Two params |
| 11 | `stage16_34_chained_calls_type_check` | Chained calls |
| 12 | `stage16_34_closure_returning_closure_with_param` | Closure returning closure |

## 4. Verification

- All 7792 tests pass (no regressions)
- Runtime: f(10)=11 ✅, f()()()=42 ✅, mut_cap=3 ✅
- No deprecated closure APIs remain

## 5. References

- Stage 16.34 design: `docs/develop/v0/stage-16/stage-16.34-cleanup-inline-closure-path.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md`
