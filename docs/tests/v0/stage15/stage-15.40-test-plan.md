# Stage 15.40 — Test Plan: Kill-on-Redefined + Driver Switch

> **Date**: 2026-08-01
> **Version**: v0.165.0 → v0.166.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.40-kill-on-redef-and-driver-switch.md`

## 1. Test Scope

Stage 15.40 completes the NLL fixpoint migration:
1. Revised `kill_expired_borrows_dataflow` to use last-use-based kill
   (fixes the `&mut self` method-call false positive).
2. Added `kill_borrows_on_redefinition` (kills borrows when ref_local
   is re-assigned).
3. Switched the driver to `check_mir_body_with_dataflow`.
4. Updated `check_crate` to use the dataflow path internally.

| Area | Test type | Count |
|------|-----------|-------|
| False positive fixed (state machine, simple, multiple method calls) | Integration | 3 |
| Driver uses dataflow path | Integration | 2 |
| Parity on all patterns | Integration | 3 |
| **Total new** | | **8** |
| Updated existing tests | | 1 (`stage15_39_known_limitation` → asserts fixed) |
| Regression (existing tests) | All | 208 lib + 2061 integration + 5216 conformance |

## 2. Integration Test Module

**Path**: `tests/v0/stage15/plan/stage15_40_driver_switch_tests.rs`
**Registered as**: `stage15_40_driver_switch_tests` (in `tests/all_tests.rs`)

### 2.1 Part A — False positive fixed (3 tests)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_40_state_machine_false_positive_fixed` | Full state machine pattern (was the 1 DATAFLOW-STRICTER case) now compiles cleanly |
| 2 | `stage15_40_simple_method_call_in_loop` | Simple `&mut self` in loop works |
| 3 | `stage15_40_multiple_method_calls_in_loop` | Multiple method calls in loop work |

### 2.2 Part B — Driver uses dataflow path (2 tests)

| # | Test name | Verifies |
|---|-----------|----------|
| 4 | `stage15_40_driver_uses_dataflow_path` | Valid program compiles via driver (which now uses dataflow path) |
| 5 | `stage15_40_driver_preserves_gap1` | Driver still rejects GAP-1 patterns (Option B's `ever_read` check is active) |

### 2.3 Part C — Parity on all patterns (3 tests)

| # | Test name | Verifies |
|---|-----------|----------|
| 6 | `stage15_40_parity_valid_borrow` | Both paths accept valid borrow |
| 7 | `stage15_40_parity_gap1_pattern` | Both paths reject GAP-1 (double-mut-borrow) |
| 8 | `stage15_40_parity_loop_borrow` | Both paths accept loop-carried borrow |

## 3. Updated Existing Tests

### 3.1 `stage15_39_known_limitation_mut_self_method_call_in_loop`

**Before Stage 15.40**: Documented the false positive (dataflow rejects,
legacy accepts). Logged the error but didn't assert on dataflow behavior.

**After Stage 15.40**: Updated to assert `dataflow_errors.is_empty()`
—the false positive is fixed. Both paths now accept the pattern.

## 4. Regression Test Strategy

### 4.1 Conformance tests (5216) — zero regression

The driver now uses the dataflow path. All 5216 conformance tests must
pass. This was verified:

```
Results: 5216 passed, 0 failed, 5216 total
ALL TESTS PASSED
```

### 4.2 Diagnostic tool — both paths agree

Re-ran the Stage 15.38 diagnostic tool after Stage 15.40:

```
  AGREE-OK:           4830
  AGREE-ERROR:        198
  LEGACY-STRICTER:    0
  DATAFLOW-STRICTER:  0
  DIFFERENT-ERRORS:   0
```

Both paths agree on all 5028 comparable conformance tests.

## 5. Expected Results

- **Stage 15.40 tests**: 8/8 PASS
- **Lib tests**: 208/208 PASS (zero regression)
- **Integration tests**: 2069/2069 PASS (2061 + 8 new, zero regression)
- **Conformance tests**: 5216/5216 PASS (zero regression — driver switched)
- **Clippy**: 0 warnings
- **Fmt**: clean
