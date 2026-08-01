# Stage 15.36 — Test Plan: `kill_expired_borrows_dataflow`

> **Date**: 2026-08-01
> **Version**: v0.161.0 → v0.162.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3
> **Design doc**: `docs/lang-design/23-nll-fixpoint.md`
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.36-kill-expired-borrows-dataflow.md`

## 1. Test Scope

Stage 15.36 adds the dataflow-driven borrow checker entry point
(`check_mir_body_with_dataflow`) and supporting APIs
(`kill_expired_borrows_dataflow`, `compute_live_after_point`,
`active_ref_locals`). The legacy `check_mir_body` is retained — the
driver still uses it, so conformance behavior is unchanged from v0.161.0.

This test plan validates:
1. **Algorithm internals** — `compute_live_after_point` correctly
   back-propagates from `LiveOut[bb]` through remaining statements.
2. **BorrowSet API** — `active_ref_locals` returns the correct dedup'd
   set, including after `kill_borrows_of_local`.
3. **Real-pipeline integration** — `check_mir_body_with_dataflow`
   accepts whatever MIR the compiler produces and converges without
   panicking.
4. **Parity** — for valid programs, the dataflow path and the legacy
   path produce the SAME error set (both empty). This is the key
   acceptance criterion: the dataflow path is a strict improvement
   (fixes loops/conditionals) but must not regress on straight-line code.
5. **Soundness patterns** — the dataflow path correctly handles borrow
   patterns where the legacy path is unsound (loop-carried borrows,
   branch-carried borrows).

| Area | Test type | Count |
|------|-----------|-------|
| `compute_live_after_point` algorithm | Unit | 5 |
| `BorrowSet::active_ref_locals` API | Unit | 4 |
| Smoke (compile + call dataflow path) | Integration | 4 |
| Parity (dataflow vs legacy) | Integration | 4 |
| Soundness (loop/branch borrows) | Integration | 2 |
| `compute_live_after_point` integration | Integration | 3 |
| **Total new** | | **22** |
| Regression (existing tests) | All | 173 lib + 2026 integration + 5216 conformance |

## 2. Unit Test Module

### 2.1 `compute_live_after_point` tests (5, in `src/borrowck/liveness.rs::tests`)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_36_compute_live_after_point_at_terminator` | At stmt_idx == statements.len(), result equals LiveOut[bb_id] (no remaining statements to fold) |
| 2 | `stage15_36_compute_live_after_point_folds_terminator` | At stmt_idx < term_idx, the terminator's Use/Def is folded into the live set |
| 3 | `stage15_36_compute_live_after_point_back_propagates` | Multiple statements after `stmt_idx` are correctly back-propagated (each one's Use/Def is folded) |
| 4 | `stage15_36_compute_live_after_point_out_of_range_bb` | Out-of-range `bb_id` returns empty set (defensive — no panic) |
| 5 | `stage15_36_compute_live_after_point_terminator_equals_live_out` | At term_idx, result equals LiveOut[bb_id] exactly (set equality, not just subset) |

### 2.2 `BorrowSet::active_ref_locals` tests (4, in `src/borrowck/borrow_set.rs::tests`)

| # | Test name | Verifies |
|---|-----------|----------|
| 6 | `stage15_36_active_ref_locals_returns_distinct_set` | Returns distinct set of ref_locals (dedup when same ref_local appears in multiple borrows) |
| 7 | `stage15_36_active_ref_locals_skips_none` | Borrows with `ref_local = None` (added via `add_borrow` without `_with_ref`) are skipped |
| 8 | `stage15_36_active_ref_locals_empty_when_no_borrows` | Empty iterator when no borrows are active |
| 9 | `stage15_36_active_ref_locals_after_kill` | After `kill_borrows_of_local(local)`, that local no longer appears in `active_ref_locals` |

## 3. Integration Test Module

**Path**: `tests/v0/stage15/plan/kill_borrows_dataflow_tests.rs`
**Registered as**: `stage15_kill_borrows_dataflow_tests` (in `tests/all_tests.rs`)

### 3.1 Part A — Smoke tests (4)

Compile real Landin source via `compile()`, then call
`check_mir_body_with_dataflow` on each MIR body. Verifies the new
analysis accepts whatever MIR the compiler produces and produces 0
errors on valid programs.

| # | Test name | Source pattern |
|---|-----------|----------------|
| 1 | `stage15_36_smoke_test_straight_line` | `let x = 1; let y = 2; x + y` |
| 2 | `stage15_36_smoke_test_control_flow` | nested `if/else if/else` |
| 3 | `stage15_36_smoke_test_loop` | `while` loop with mutation |
| 4 | `stage15_36_smoke_test_borrows` | `let r = &x; *r + 0` |

### 3.2 Part B — Parity tests (4, dataflow vs legacy)

For each compiled MIR body, both paths must produce the SAME error set
on valid programs (both should be empty). This is the key Stage 15.36
acceptance criterion: the dataflow path is a strict improvement but
must not regress on existing code.

| # | Test name | Source pattern | Asserts |
|---|-----------|----------------|---------|
| 5 | `stage15_36_parity_simple_program` | straight-line `let x = 1; let y = 2; let z = x + y; z` | legacy.len() == dataflow.len() |
| 6 | `stage15_36_parity_control_flow_program` | `abs(n) = if n < 0 { -n } else { n }` | legacy == 0 && dataflow == 0 |
| 7 | `stage15_36_parity_loop_program` | `sum_to(n) = while i < n { s += i; i += 1; }` (no borrows) | legacy == 0 && dataflow == 0 |
| 8 | `stage15_36_parity_borrows_straight_line` | `let r = &x; let y = *r; y` | legacy == 0 && dataflow == 0 |

### 3.3 Part C — Soundness tests (2, where legacy may fail)

These are the patterns the dataflow path was designed to fix. The
dataflow path must produce 0 errors. (We don't assert the legacy path
produces errors — its behavior depends on MIR lower internals and may
vary. The dataflow path is the source of truth.)

| # | Test name | Source pattern | Why legacy may fail |
|---|-----------|----------------|---------------------|
| 9 | `stage15_36_loop_borrow_survives_across_iterations` | `let r = &x; while i < 3 { s += *r; i += 1; }` | Legacy `compute_last_use_map` may kill `r`'s borrow after the first iteration's "last use", causing false-positive "use of killed borrow" errors on subsequent iterations. |
| 10 | `stage15_36_branch_borrow_survives_both_arms` | `let r = &x; let y = if *r > 5 { *r + 1 } else { *r - 1 };` | Legacy doesn't track branch liveness — may kill `r`'s borrow in one arm before the other arm reads it. |

### 3.4 Part D — `compute_live_after_point` integration tests (3)

| # | Test name | Verifies |
|---|-----------|----------|
| 11 | `stage15_36_compute_live_after_point_smoke_test` | Callable on real MIR, no panic for any program point (every block, every stmt_idx 0..=statements.len()) |
| 12 | `stage15_36_compute_live_after_point_terminator_eq_live_out` | At stmt_idx == statements.len() (terminator), result equals LiveOut[bb] exactly |
| 13 | `stage15_36_smoke_test_complex_program` | Mixed borrows + loops + conditionals — dataflow path produces 0 errors |

## 4. Regression Test Strategy

### 4.1 No regression expected

Stage 15.36 adds new code only — it does not modify the existing borrow
checker walk. The legacy `check_mir_body` is still the active analysis
in the driver. All 173 lib tests + 2026 integration tests + 5216
conformance tests must pass unchanged.

### 4.2 Conformance tests

All 5216 conformance tests must continue to pass. The dataflow borrow
checker is not invoked by the driver in this stage — it's only invoked
by the new tests. So conformance behavior is identical to v0.161.0.

### 4.3 Public API additions

The new public symbols (`check_mir_body_with_dataflow`,
`compute_live_after_point`, `active_ref_locals`) are added to the
`borrowck` module's re-export list. No existing public symbols are
removed or renamed — only additions.

## 5. Coverage Matrix

| Feature | Unit tests | Integration tests | Total |
|---------|-----------|-------------------|-------|
| `compute_live_after_point` algorithm (5 edge cases) | 5 | 3 (smoke + terminator_eq) | 8 |
| `BorrowSet::active_ref_locals` API (4 cases) | 4 | 0 (covered by dataflow integration) | 4 |
| `check_mir_body_with_dataflow` smoke (4 source patterns) | 0 | 4 | 4 |
| Parity (dataflow vs legacy, 4 source patterns) | 0 | 4 | 4 |
| Soundness (loop/branch borrows) | 0 | 2 | 2 |
| **Total** | **9** | **13** | **22** |

## 6. Negative Test Coverage (§9.1.1)

Stage 15.36 doesn't introduce user-facing error messages — the dataflow
borrow checker reports the same `BorrowError` types as the legacy path.
However, the tests do cover the following negative/edge scenarios:

| Scenario | Test |
|----------|------|
| Empty live set (no locals live after point) | `stage15_36_compute_live_after_point_at_terminator` |
| Out-of-range bb_id (defensive — no panic) | `stage15_36_compute_live_after_point_out_of_range_bb` |
| Empty BorrowSet (no active borrows) | `stage15_36_active_ref_locals_empty_when_no_borrows` |
| Borrows with `ref_local = None` (skipped, not crashed) | `stage15_36_active_ref_locals_skips_none` |
| Borrow killed mid-analysis (after `kill_borrows_of_local`) | `stage15_36_active_ref_locals_after_kill` |
| Loop-carried borrow (soundness — legacy may fail) | `stage15_36_loop_borrow_survives_across_iterations` |
| Branch-carried borrow (soundness — legacy may fail) | `stage15_36_branch_borrow_survives_both_arms` |

## 7. Test Execution

```bash
# Run only the new unit tests
cargo test --features llvm-backend --lib borrowck::liveness::tests::stage15_36
cargo test --features llvm-backend --lib borrowck::borrow_set::tests::stage15_36

# Run only the new integration tests
cargo test --features llvm-backend --test all_tests stage15_kill_borrows_dataflow_tests

# Run all tests (regression check)
cargo test --features llvm-backend

# Run conformance tests
python3 tests/conformance/run_all.py
```

## 8. Expected Results

- **Unit tests**: 9/9 PASS (5 + 4)
- **Integration tests**: 13/13 PASS
- **Lib tests**: 182/182 PASS (173 + 9 new, no regression)
- **Existing integration tests**: 2026/2026 PASS (no regression)
- **Conformance tests**: 5216/5216 PASS (no regression)
- **Clippy**: 0 warnings
- **Fmt**: clean

## 9. Stage Gate Review — Test Coverage (§29.1.3 Design-Impl-Test)

| Design point (from `23-nll-fixpoint.md` §2.3) | Implementation | Test |
|-----------------------------------------------|----------------|------|
| `kill_expired_borrows_dataflow` using liveness maps | `src/borrowck/mod.rs` | Parity tests + soundness tests |
| `check_mir_body_with_dataflow` entry point | `src/borrowck/mod.rs` | All 13 integration tests |
| Per-statement liveness (`compute_live_after_point`) | `src/borrowck/liveness.rs` | 5 unit + 3 integration tests |
| `BorrowSet::active_ref_locals` (kill path enumeration) | `src/borrowck/borrow_set.rs` | 4 unit tests |
| Backward-compat with legacy `check_mir_body` | `src/borrowck/mod.rs` (both paths retained) | All existing tests pass (regression check) |
| Migration step 2 of 4 (15.34→15.37) | This stage adds API; driver switch in 15.37 | N/A (driver still uses legacy) |
| §23 API naming compliance | All new symbols follow conventions | Manual review (passed) |

All design points have implementation and tests. No "design requires but
not implemented" or "implemented but not tested" gaps.
