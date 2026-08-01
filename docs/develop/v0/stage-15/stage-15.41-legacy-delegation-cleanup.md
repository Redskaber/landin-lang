# Stage 15.41 — Legacy Delegation Cleanup (Dead Code Removal)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.166.0 → v0.167.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 2 Task 7 (cleanup)**: NLL migration cleanup — legacy code removal
> **Prior stage**: `docs/develop/v0/stage-15/stage-15.40-kill-on-redef-and-driver-switch.md`

## 1. Executive Summary

Stage 15.41 completes the NLL migration cleanup. The legacy `check_mir_body`
(method + free function) now delegates directly to
`check_mir_body_with_dataflow`. The original single-pass walk implementation
(`kill_expired_borrows` + the legacy `check_mir_body` body) has been removed
as dead code.

**Key results**:
- Removed ~60 LOC of dead code (the legacy `kill_expired_borrows` method +
  the legacy `check_mir_body` walk body).
- The legacy `check_mir_body` API is retained as `#[deprecated]` for
  backward compatibility with ~15 test files that still call it.
- `compute_last_use_map` is retained — it's now part of the dataflow path
  (Stage 15.40 revised the kill logic to use last-use-based kill).
- All 208 lib + 2076 integration + 5216 conformance tests pass (zero regression).

Per §1.0 原則 1 "长期 > 短期": delegating to the dataflow path eliminates
the dead code while preserving the API for callers that haven't migrated.
Per §15 "最优 > 最小": the legacy walk body is removed (not retained as
dead code), reducing maintenance burden.

## 2. What Was Done

### 2.1 Legacy `check_mir_body` now delegates to dataflow path

**Before Stage 15.41** (the legacy `check_mir_body` method):
```rust
pub fn check_mir_body(&mut self, mir: &MirBody) {
    let last_use_map = compute_last_use_map(mir);
    for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
        // ... 30+ lines of walk logic using kill_expired_borrows ...
    }
    self.run_region_inference(mir);
}
```

**After Stage 15.41**:
```rust
#[deprecated(note = "Use `check_mir_body_with_dataflow` ...")]
pub fn check_mir_body(&mut self, mir: &MirBody) {
    // Stage 15.41: Delegate directly to the dataflow path.
    self.check_mir_body_with_dataflow(mir);
}
```

The free function `check_mir_body(mir: &MirBody) -> Vec<BorrowError>` was
already calling `bc.check_mir_body(mir)`, so it now also delegates to the
dataflow path (via the method).

### 2.2 Removed `kill_expired_borrows` (the legacy walk version)

The `kill_expired_borrows` method (the single-pass walk version that took
a `LastUseMap` and killed borrows at their last-use point) has been removed.
It was only called by the legacy `check_mir_body` walk, which is now
replaced by delegation.

Note: `kill_expired_borrows_dataflow` (the dataflow version) is retained —
it's the active kill path used by `check_mir_body_with_dataflow`.

### 2.3 Updated `compute_last_use_map` documentation

The `compute_last_use_map` function's documentation was updated to clarify
that it's NO LONGER legacy — it's now part of the dataflow borrow-check
path. Stage 15.40 revised `kill_expired_borrows_dataflow` to use
last-use-based kill (borrow lifetimes end at their last read), which
requires this map.

The original "unsound for loops" concern was about using this map for
LOCAL liveness, but it's correct for BORROW lifetimes (a borrow's useful
lifetime ends at its last read, regardless of loop structure).

### 2.4 What was NOT removed

- `compute_last_use_map` — retained (used by the dataflow path).
- `LastUseMap` type alias — retained (used by `compute_last_use_map`).
- `compute_liveness`, `LiveInMap`, `LiveOutMap` — retained for future use
  (full NLL with borrow regions). Not currently used for the kill decision,
  but the infrastructure is valuable.
- `compute_live_after_point` — retained (was used by Stage 15.36-15.39
  liveness-based kill; now unused but kept for future use).
- `compute_ever_read` — retained (used by the dataflow path for GAP-1
  preservation, Stage 15.39 Option B).
- Legacy `check_mir_body` (method + free fn) — retained as `#[deprecated]`
  for backward compatibility with ~15 test files.

## 3. Why Delegation Instead of Removal

The legacy `check_mir_body` API is still used by ~15 test files:
- `tests/v0/stage7/plan/{design_writeback_verification,deep_review,systematic_review_v014,region_inference}_tests.rs`
- `tests/v0/stage8/plan/{lifetime_elision,deep_review,drop_elaboration}_tests.rs`
- `tests/v0/stage15/plan/{stage15_37_driver_switch_tests,kill_borrows_dataflow_tests,borrowck_comparison_diagnostic_tests,option_b_implementation_tests}.rs`

Removing the API would break all these tests. Per §1.0 原則 1 "长期 > 短期":
delegating to the dataflow path preserves the API (short-term compat) while
eliminating the dead code (long-term cleanliness). The API will be fully
removed in v0.3 when the tests are migrated.

## 4. Verification

- `cargo build --features llvm-backend` — ✅ clean build, 0 warnings
- `cargo test --features llvm-backend` — ✅ 208 lib + 2076 integration = 2284 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- 0 clippy warnings, fmt clean

## 5. Testing

### 5.1 New integration tests (7, in `tests/v0/stage15/plan/stage15_41_legacy_delegation_tests.rs`)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_41_legacy_free_fn_delegates_to_dataflow` | Legacy free fn produces same results as dataflow (GAP-1 pattern) |
| 2 | `stage15_41_legacy_method_delegates_to_dataflow` | Legacy method produces same results as dataflow (valid borrow) |
| 3 | `stage15_41_compute_last_use_map_still_available` | `compute_last_use_map` is still callable (part of dataflow path) |
| 4 | `stage15_41_legacy_accepts_valid_borrow` | Legacy API accepts valid borrow |
| 5 | `stage15_41_legacy_rejects_gap1` | Legacy API rejects GAP-1 (delegates to dataflow) |
| 6 | `stage15_41_legacy_accepts_loop_borrow` | Legacy API accepts loop-carried borrow |
| 7 | `stage15_41_legacy_accepts_method_call_in_loop` | Legacy API accepts `&mut self` method call in loop (false positive fixed) |

## 6. Migration Plan (Stages 15.34-15.41) — COMPLETE

| Stage | Status | Description |
|-------|--------|-------------|
| 15.34 | ✅ DONE (v0.160.0) | NLL fixpoint design doc |
| 15.35 | ✅ DONE (v0.161.0) | `compute_liveness` fixpoint function |
| 15.36 | ✅ DONE (v0.162.0) | `kill_expired_borrows_dataflow` + `check_mir_body_with_dataflow` |
| 15.37 | ⚠️ PARTIAL (v0.163.0) | Legacy `check_mir_body` deprecated; driver switch DEFERRED |
| 15.38 | ✅ DONE (v0.164.0) | Diagnostic tool + reconciliation design doc |
| 15.39 | ✅ DONE (v0.165.0) | Option B: GAP-1 preserved (112 → 0) |
| 15.40 | ✅ DONE (v0.166.0) | Kill-on-redef + driver switch (NLL migration COMPLETE) |
| **15.41** | **✅ DONE (v0.167.0)** | **Legacy delegation cleanup — dead code removed (this stage)** |

**The NLL fixpoint migration (Phase 2 Task 7, HP-10) is FULLY COMPLETE.**

## 7. Stage Gate Review (self-review per §9.3)

| Check | Status |
|-------|--------|
| Legacy `check_mir_body` delegates to dataflow path | ✅ |
| Dead code removed (`kill_expired_borrows` legacy walk) | ✅ |
| `compute_last_use_map` retained (used by dataflow path) | ✅ |
| All existing tests pass (zero regression) | ✅ |
| API naming compliance (§23) | ✅ |
| 0 clippy warnings | ✅ |
| fmt clean | ✅ |

## 8. Conclusion

Stage 15.41 completes the NLL migration cleanup. The legacy `check_mir_body`
API now delegates directly to `check_mir_body_with_dataflow`, eliminating
~60 LOC of dead code (the legacy walk implementation). The API is retained
as `#[deprecated]` for backward compatibility with existing tests.

The NLL fixpoint migration (Stages 15.34-15.41) is **fully complete**:
- The driver uses the dataflow-driven borrow checker.
- The legacy API delegates to the dataflow path (no behavior difference).
- Dead code is removed.
- All 5216 conformance tests pass.
- All 2284 rust tests pass.

The `compute_liveness` infrastructure (Stages 15.35-15.36) is retained
for future use (full NLL with borrow regions), but the current kill
decision uses the last-use-based approach + `ever_read` check + kill-on-
redefinition.

**Phase 2 Task 7 (HP-10) is CLOSED.**
