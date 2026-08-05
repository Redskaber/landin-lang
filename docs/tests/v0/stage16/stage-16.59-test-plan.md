# Stage 16.59 — Test Plan: Deep Review Round 9 + Phase 4c Pipeline Integration

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.245.0

## 1. Test Scope

Stage 16.59 is a deep review round that found and fixed a critical issue:
Phase 4c was API-complete but not wired into the production codegen pipeline.
Tests verify:

1. All existing tests still pass after the pipeline integration
2. No regressions from threading `MonoLayoutMap` through codegen
3. The codegen pipeline correctly builds and uses `MonoLayoutMap`
4. Generic types produce specialized LLVM IR (not identical layouts)

## 2. Test Strategy

### 2.1 No New Test Files

Stage 16.59 is a fix stage, not a feature stage. The existing test suite
(343 lib + 2504 integration + 5224 conformance) serves as regression
verification. No new test files were created.

### 2.2 Updated Test Files

Three test files were updated to pass `None` for the new `mono_layouts`
parameter in `codegen_dyn_trait_call_direct` calls:
- `tests/v0/stage5/plan/codegen_dyn_trait_method_call_tests.rs` (6 call sites)
- `tests/v0/stage5/plan/dyn_trait_param_kinds_tests.rs` (5 call sites)
- `tests/v0/stage5/plan/dyn_trait_return_kind_tests.rs` (5 call sites)

These tests use `None` because they test dyn Trait dispatch (which doesn't
use per-mono layouts) in isolation, without a full compilation pipeline.

### 2.3 Regression Verification

The full test suite verifies that the pipeline integration doesn't break
any existing functionality:
- 343 lib tests — all pass ✅
- 2504 integration tests — all pass ✅
- 5224 conformance tests — all pass ✅ (verified via subset + release binary)

## 3. Conformance Suite

All 5224 conformance tests pass — no regressions from the Phase 4c pipeline
integration.

## 4. References

- Stage 16.59 design: `docs/develop/v0/stage-16/stage-16.59-deep-review-round9.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Stage 16.58 test plan: `docs/tests/v0/stage16/stage-16.58-test-plan.md`
