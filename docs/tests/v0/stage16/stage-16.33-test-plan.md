# Stage 16.33 — Test Plan: Deep Review Round 6 (v0.3 Milestone Verification)

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.230.3
> **Process**: stage-committee-process.md v3.24 §17.5 + §25 + §29.1.3

## 1. Test Scope

Stage 16.33 is the v0.3 closure redesign completion review. The test plan
verifies that all v0.3 milestones are achieved and stable:

1. All closure patterns compile (no-capture, i32/struct/mutable captures, nested)
2. Sound Copy detection works end-to-end
3. Task 3 DefId-keyed lookup is consistent
4. Complete program with all v0.3 features compiles

## 2. Test File

- `tests/v0/stage16/plan/stage16_33_deep_review_round6_tests.rs`
- 10 tests, all passing ✅

## 3. Test Matrix

| # | Test Name | Verifies |
|---|-----------|----------|
| 1 | `stage16_33_nocapture_closure_complete` | No-capture closure (synthesized path) |
| 2 | `stage16_33_i32_capture_complete` | i32-capture closure (typeck gap fixed) |
| 3 | `stage16_33_struct_capture_complete` | Struct-capture closure (was inline path) |
| 4 | `stage16_33_mutable_capture_complete` | Mutable-capture closure (borrowck works) |
| 5 | `stage16_33_nested_closure_complete` | Double-nested closure (codegen works) |
| 6 | `stage16_33_triple_nested_complete` | Triple-nested closure (typeck works) |
| 7 | `stage16_33_sound_copy_derived` | Sound Copy — derived Copy works |
| 8 | `stage16_33_sound_copy_non_copy` | Sound Copy — non-Copy rejects double-move |
| 9 | `stage16_33_def_id_lookup` | Task 3 — DefId-keyed lookup |
| 10 | `stage16_33_complete_program_all_features` | All v0.3 features together |

## 4. Runtime Verification

| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| `f(10)` (no-capture) | 11 | 11 | ✅ |
| `x + y` (i32 capture) | 15 | 15 | ✅ |
| `f()()` (double-nested) | 42 | 42 | ✅ |
| `f()()()` (triple-nested) | 42 | 42 | ✅ |
| `f() = 3` (mutable capture loop) | 3 | 3 | ✅ |

## 5. v0.3 Release Decision

**v0.3 RELEASE APPROVED** — Deep Review Round 6, 5/5 committee vote GO.

All closure TDs closed. All runtime patterns verified. Architecture follows
通解 principle. API naming compliant with §23.

## 6. References

- Deep review report: `docs/develop/v0/stage-16/deep-review-round6.md`
- Stage 16.33 design: `docs/develop/v0/stage-16/stage-16.33-deep-review-round6.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md`
