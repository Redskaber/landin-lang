# Stage 15.41 — Test Plan: Legacy Delegation Cleanup

> **Date**: 2026-08-01
> **Version**: v0.166.0 → v0.167.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.41-legacy-delegation-cleanup.md`

## 1. Test Scope

Stage 15.41 completes the NLL migration cleanup:
1. Legacy `check_mir_body` (method + free fn) now delegates to
   `check_mir_body_with_dataflow`.
2. Removed dead code: `kill_expired_borrows` (legacy walk version).
3. Updated `compute_last_use_map` documentation (now part of dataflow path).

| Area | Test type | Count |
|------|-----------|-------|
| Legacy API delegates to dataflow (same results) | Integration | 2 |
| `compute_last_use_map` still available | Integration | 1 |
| No behavior change (all patterns work) | Integration | 4 |
| **Total new** | | **7** |
| Regression (existing tests) | All | 208 lib + 2076 integration + 5216 conformance |

## 2. Integration Test Module

**Path**: `tests/v0/stage15/plan/stage15_41_legacy_delegation_tests.rs`
**Registered as**: `stage15_41_legacy_delegation_tests` (in `tests/all_tests.rs`)

### 2.1 Part A — Legacy API delegates to dataflow path (2 tests)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_41_legacy_free_fn_delegates_to_dataflow` | Legacy free fn produces same error count as dataflow (GAP-1 pattern — both reject) |
| 2 | `stage15_41_legacy_method_delegates_to_dataflow` | Legacy method produces same error count as dataflow (valid borrow — both accept) |

### 2.2 Part B — `compute_last_use_map` still available (1 test)

| # | Test name | Verifies |
|---|-----------|----------|
| 3 | `stage15_41_compute_last_use_map_still_available` | `compute_last_use_map` is callable on real MIR (no panic) |

### 2.3 Part C — No behavior change (4 tests)

| # | Test name | Verifies |
|---|-----------|----------|
| 4 | `stage15_41_legacy_accepts_valid_borrow` | Legacy API accepts valid borrow |
| 5 | `stage15_41_legacy_rejects_gap1` | Legacy API rejects GAP-1 (delegates to dataflow) |
| 6 | `stage15_41_legacy_accepts_loop_borrow` | Legacy API accepts loop-carried borrow |
| 7 | `stage15_41_legacy_accepts_method_call_in_loop` | Legacy API accepts `&mut self` method call in loop (false positive fixed) |

## 3. Regression Test Strategy

### 3.1 Conformance tests (5216) — zero regression

The driver uses the dataflow path (Stage 15.40). The legacy API now
delegates to the dataflow path. All 5216 conformance tests must pass.
Verified:

```
Results: 5216 passed, 0 failed, 5216 total
ALL TESTS PASSED
```

### 3.2 Existing tests that call legacy `check_mir_body` (~15 files)

These tests call the legacy `check_mir_body` API. Since the legacy API
now delegates to the dataflow path (which produces identical results),
these tests should pass unchanged. All 2076 integration tests pass.

## 4. Expected Results

- **Stage 15.41 tests**: 7/7 PASS
- **Lib tests**: 208/208 PASS (zero regression)
- **Integration tests**: 2083/2083 PASS (2076 + 7 new, zero regression)
- **Conformance tests**: 5216/5216 PASS (zero regression)
- **Clippy**: 0 warnings
- **Fmt**: clean
