# Stage 15.65 — Test Plan: HP-22 Cleanup (Remove Legacy dyn_trait_calls Side-Table)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.190.0 → v0.191.0
> **Process**: stage-committee-process.md v3.23 §17.5

## 1. Test Categories

### 1.1 Regression — Conformance suite (5216 tests)
**Expected**: All 5216 pass (no regression from side-table removal).

### 1.2 Updated — Integration tests (6 files)
**Files updated** to use `codegen_dyn_trait_call_direct` and verify via
terminator's `dyn_trait_call` field:

| File | Changes |
|------|---------|
| `dyn_trait_return_kind_tests.rs` | Use `_direct` variant; build `DynTraitMethodCall` directly |
| `dyn_trait_param_kinds_tests.rs` | Use `_direct` variant; build `DynTraitMethodCall` directly |
| `codegen_dyn_trait_method_call_tests.rs` | Use `_direct` variant; removed OOB panic test |
| `mir_lower_dyn_trait_method_call_integration_tests.rs` | Verify via terminator field |
| `driver_dyn_trait_plan_integration_tests.rs` | Count via `TerminatorKind::Call { dyn_trait_call: Some(_) }` |
| `dyn_trait_e2e_integration_tests.rs` | Count via terminator field |

### 1.3 Runtime — dyn Trait programs still work
All conformance tests with `dyn Trait` pass — the codegen path uses the
`dyn_trait_call` field (Stage 15.30), not the removed side-table.

## 2. Sign-off
- ✅ All 5216 conformance tests pass.
- ✅ All 2125 integration tests pass.
- ✅ All 226 lib tests pass.
- ✅ 0 clippy warnings, fmt clean.

**Total: 7567 tests passing, 0 failures.**

Stage 15.65 is GO for merge.
