# Stage 15.40 — Kill-on-Redefined + Driver Switch (NLL Migration COMPLETE)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.165.0 → v0.166.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 2 Task 7 (COMPLETE)**: NLL fixpoint migration — driver switched
> **Prior stage**: `docs/develop/v0/stage-15/stage-15.39-option-b-implementation.md`

## 1. Executive Summary

Stage 15.40 **completes the NLL fixpoint migration** (Stages 15.34-15.40).
The driver now uses `check_mir_body_with_dataflow` instead of the legacy
`check_mir_body`. The `&mut self` method-call false positive (the 1
DATAFLOW-STRICTER case from Stage 15.39) is fixed.

**Key results**:
- Diagnostic tool confirms: LEGACY-STRICTER = 0, DATAFLOW-STRICTER = 0
  (both paths agree on all 5028 comparable conformance tests).
- All 5216 conformance tests pass with the driver using the dataflow path.
- All 208 lib + 2061 integration tests pass (zero regression).

The NLL migration is **complete**. The legacy `check_mir_body` remains
as `#[deprecated]` for backward compatibility; Stage 15.41 will remove
it (now truly dead code).

Per §1.0 原則 1 "长期 > 短期": the dataflow path is the correct
long-term design. Per §1.0 原則 3 "显式 > 隐式": the choice of analysis
is explicit in the method name.

## 2. What Was Done

### 2.1 Revised `kill_expired_borrows_dataflow` (last-use-based kill)

Stage 15.36-15.39 used liveness-based kill (`compute_live_after_point` +
`LiveOutMap`). This was correct for **local** lifetimes but wrong for
**borrow** lifetimes. A borrow temp in a loop is correctly live across
the back-edge (its value is re-assigned each iteration), but the
**borrow** should expire at the call that uses it (the borrow's last
read), not at the local's last use (the re-assignment).

Stage 15.40 revised `kill_expired_borrows_dataflow` to use
`compute_last_use_map` (same as the legacy path) for the kill decision.
The kill logic is now:

1. If `ref_local` was never read → don't kill (GAP-1 preservation,
   Stage 15.39 Option B).
2. If `ref_local`'s last read is at the current point → kill (borrow's
   lifetime ends at its last read, standard NLL borrow-lifetime semantics).

The `compute_live_after_point` and `LiveOutMap` infrastructure is
retained for future use (e.g., full NLL with borrow regions) but is no
longer used for the kill decision.

### 2.2 Added `kill_borrows_on_redefinition`

A new method that kills any active borrow whose `ref_local` is the LHS
of the current `Assign` statement. This handles the case where a borrow
temp is re-assigned — the old borrow is stale and must be killed before
the new borrow is created.

This is called before `check_statement` in the dataflow walk.

### 2.3 Switched the driver

`src/driver.rs` now calls `bc.check_mir_body_with_dataflow(&mir)` instead
of `bc.check_mir_body(&mir)`. The `#[allow(deprecated)]` attributes at
the call site are removed (no longer needed).

### 2.4 Updated `check_crate`

The deprecated `check_crate` free function now calls
`check_mir_body_with_dataflow` internally (matching the driver's behavior).

### 2.5 Updated existing tests

- `stage15_39_known_limitation_mut_self_method_call_in_loop` — updated
  to assert `dataflow_errors.is_empty()` (the false positive is fixed).
- `stage15_37_gap1_semantic_conflict_documented` — already updated in
  Stage 15.39 (asserts both paths reject GAP-1).

## 3. Why the Last-Use-Based Kill Fixes the False Positive

### 3.1 The false positive root cause

In a loop like:
```rust
while i < 5 {
    c.increment();  // lowers to: tmp = &mut c; call increment(tmp); ...
    i = i + 1;
}
```

Each iteration creates a fresh `tmp = &mut c`. The liveness analysis
correctly identifies `tmp` as live across the loop back-edge (it's
used in the next iteration's call). This means the liveness-based kill
never kills `tmp`'s borrow — it stays alive and conflicts with the
next iteration's `&mut c` borrow.

### 3.2 The last-use-based kill fix

The `compute_last_use_map` records `tmp`'s last read at the call point
(within the same iteration). The last-use-based kill kills `tmp`'s
borrow at the call's terminator — before the next iteration's `tmp = &mut c`
is processed. This correctly expires the borrow at its last use, not at
the local's re-assignment.

### 3.3 Why GAP-1 is still preserved

The `ever_read` check (Stage 15.39 Option B) is still active. A borrow
whose `ref_local` was never read (like `r1` in
`let r1 = &mut x; let r2 = &mut x;`) is NOT killed — it stays as a
"stray" until scope end, matching the legacy path's behavior. This
preserves GAP-1 soundness.

## 4. Diagnostic Verification

Re-ran the Stage 15.38 diagnostic tool after Stage 15.40:

```
Files scanned: 5216 (skipped: 188)
Files compared: 5028
  AGREE-OK:           4830  (was 4829 — 1 case moved from DATAFLOW-STRICTER)
  AGREE-ERROR:        198
  LEGACY-STRICTER:    0     (was 112 — GAP-1 conflict resolved by Option B)
  DATAFLOW-STRICTER:  0     (was 1 — false positive fixed by Stage 15.40 ✅)
  DIFFERENT-ERRORS:   0
```

**Both paths now agree on all 5028 comparable conformance tests.**
The NLL migration is complete.

## 5. Verification

- `cargo build --features llvm-backend` — ✅ clean build, 0 warnings
- `cargo test --features llvm-backend` — ✅ 208 lib + 2061 integration = 2269 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- Diagnostic tool: LEGACY-STRICTER = 0, DATAFLOW-STRICTER = 0
- 0 clippy warnings, fmt clean

## 6. Testing

### 6.1 New integration tests (8, in `tests/v0/stage15/plan/stage15_40_driver_switch_tests.rs`)

**Part A — False positive fixed (3 tests):**
1. `stage15_40_state_machine_false_positive_fixed` — full state machine pattern works
2. `stage15_40_simple_method_call_in_loop` — simple `&mut self` in loop works
3. `stage15_40_multiple_method_calls_in_loop` — multiple method calls in loop work

**Part B — Driver uses dataflow path (2 tests):**
4. `stage15_40_driver_uses_dataflow_path` — valid program compiles via driver
5. `stage15_40_driver_preserves_gap1` — driver still rejects GAP-1 patterns

**Part C — Parity on all patterns (3 tests):**
6. `stage15_40_parity_valid_borrow` — both paths accept valid borrow
7. `stage15_40_parity_gap1_pattern` — both paths reject GAP-1
8. `stage15_40_parity_loop_borrow` — both paths accept loop borrow

### 6.2 Updated existing tests

- `stage15_39_known_limitation_mut_self_method_call_in_loop` — updated
  to assert the false positive is fixed.
- `stage15_37_gap1_semantic_conflict_documented` — already updated in
  Stage 15.39.

## 7. API Naming Compliance (§23)

| Symbol | Pattern | Status |
|--------|---------|--------|
| `kill_borrows_on_redefinition` | `<verb>_<noun>_<preposition>` (private method) | ✅ |
| `kill_expired_borrows_dataflow` (revised) | `<verb>_<noun>_<noun>` with `_dataflow` suffix | ✅ |
| `check_mir_body_with_dataflow` (driver now uses this) | `<verb>_<noun>_<noun>` with `_with_dataflow` suffix | ✅ |

## 8. Migration Plan (Stages 15.34-15.41) — COMPLETE

| Stage | Status | Description |
|-------|--------|-------------|
| 15.34 | ✅ DONE (v0.160.0) | NLL fixpoint design doc |
| 15.35 | ✅ DONE (v0.161.0) | `compute_liveness` fixpoint function |
| 15.36 | ✅ DONE (v0.162.0) | `kill_expired_borrows_dataflow` + `check_mir_body_with_dataflow` |
| 15.37 | ⚠️ PARTIAL (v0.163.0) | Legacy `check_mir_body` deprecated; driver switch DEFERRED |
| 15.38 | ✅ DONE (v0.164.0) | Diagnostic tool + reconciliation design doc |
| 15.39 | ✅ DONE (v0.165.0) | Option B: GAP-1 preserved (112 → 0) |
| **15.40** | **✅ DONE (v0.166.0)** | **Kill-on-redef + driver switch (false positive fixed) — this stage** |
| 15.41 | ⏳ NEXT | Remove legacy `compute_last_use_map` + `kill_expired_borrows` + `check_mir_body` (now dead code) |

## 9. Stage Gate Review (self-review per §9.3)

| Check | Status |
|-------|--------|
| False positive fixed (DATAFLOW-STRICTER = 0) | ✅ |
| Driver switched to dataflow path | ✅ |
| All 5216 conformance tests pass | ✅ |
| GAP-1 still preserved (LEGACY-STRICTER = 0) | ✅ |
| Both paths agree on all 5028 comparable tests | ✅ |
| API naming compliance (§23) | ✅ |
| 0 clippy warnings | ✅ |
| fmt clean | ✅ |
| Zero regression on existing tests | ✅ |

## 10. Conclusion

Stage 15.40 completes the NLL fixpoint migration. The driver now uses
the dataflow-driven borrow checker (`check_mir_body_with_dataflow`).
The `&mut self` method-call false positive is fixed by revising the
kill logic to use last-use-based kill (borrow lifetimes end at their
last read) plus kill-on-redefinition (kill borrows when their
ref_local is re-assigned).

The diagnostic tool confirms both paths agree on all 5028 comparable
conformance tests. All 5216 conformance tests pass with the driver
using the dataflow path.

The legacy `check_mir_body` remains as `#[deprecated]` for backward
compatibility. Stage 15.41 will remove it (now truly dead code),
completing the NLL migration cleanup.

**The NLL migration (Phase 2 Task 7, HP-10) is COMPLETE.**
