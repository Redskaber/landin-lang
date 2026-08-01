# Stage 15.35 — Test Plan: NLL Fixpoint Liveness Analysis

> **Date**: 2026-08-01
> **Version**: v0.160.0 → v0.161.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3
> **Design doc**: `docs/lang-design/23-nll-fixpoint.md`
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.35-nll-fixpoint-liveness.md`

## 1. Test Scope

Stage 15.35 adds the fixpoint `compute_liveness` function (and helpers
`successors`, `statement_writes`, `place_root_writes`, `terminator_writes`)
to `src/borrowck/liveness.rs`. The legacy `compute_last_use_map` is
retained for backward compatibility — the borrow checker still uses it.
This test plan validates the **new** fixpoint analysis without affecting
the existing borrow-check behavior.

| Area | Test type | Count |
|------|-----------|-------|
| `successors()` — all TerminatorKind variants | Unit | 9 |
| `statement_writes` / `terminator_writes` | Unit | 3 |
| `compute_liveness` — straight-line, branch, loop, edge cases | Unit | 9 |
| Integration — synthetic CFGs with precise LiveIn/LiveOut assertions | Integration | 8 |
| Integration — real-pipeline smoke tests via `compile()` | Integration | 5 |
| **Total new** | | **34** |
| Regression (existing tests) | All | 173 lib + 2013 integration + 5216 conformance |

## 2. Unit Test Module

**Path**: `src/borrowck/liveness.rs` (inline `mod tests`)
**Coverage**: algorithm internals — `successors`, write-collection helpers,
`compute_liveness` on synthetic CFGs.

### 2.1 `successors()` test cases (9)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_35_successors_goto` | `Goto(t)` → `[t]` |
| 2 | `stage15_35_successors_return_empty` | `Return` → `[]` |
| 3 | `stage15_35_successors_unreachable_empty` | `Unreachable` → `[]` |
| 4 | `stage15_35_successors_switchint_includes_otherwise` | `SwitchInt { targets, otherwise }` → all targets + otherwise |
| 5 | `stage15_35_successors_drop_no_unwind` | `Drop { unwind: None }` → `[target]` |
| 6 | `stage15_35_successors_drop_with_unwind` | `Drop { unwind: Some(u) }` → `[target, u]` |
| 7 | `stage15_35_successors_call_with_target` | `Call { target: Some(t) }` → `[t]` |
| 8 | `stage15_35_successors_call_no_target_divergent` | `Call { target: None }` → `[]` (divergent) |
| 9 | `stage15_35_successors_assert` | `Assert { target }` → `[target]` |

### 2.2 Write-collection helper tests (3)

| # | Test name | Verifies |
|---|-----------|----------|
| 10 | `stage15_35_statement_writes_assign_lhs_root` | `Assign(place, _)` writes the root local of `place` |
| 11 | `stage15_35_terminator_writes_call_destination` | `Call { destination, .. }` writes the root local of `destination` |
| 12 | `stage15_35_terminator_writes_goto_empty` | `Goto` writes nothing |

### 2.3 `compute_liveness()` test cases (9)

| # | Test name | CFG shape | Verifies |
|---|-----------|-----------|----------|
| 13 | `stage15_35_compute_liveness_straight_line_dead` | `bb0: x=1; y=2; ret` | Dead vars (Def no Use) → empty LiveIn |
| 14 | `stage15_35_compute_liveness_straight_line_read_at_terminator` | `bb0: x=x; assert(x)` self-loop | Read in terminator → x in live_out |
| 15 | `stage15_35_compute_liveness_branch_both_arms_use_x` | `bb0: switch(x); bb1: y=x; bb2: z=x; bb3: ret` | x live in bb0, bb1, bb2 (both arms) |
| 16 | `stage15_35_compute_liveness_loop_x_live_across_iterations` | `bb0: x=x; bb1: switch(x); bb2: x=x; bb3: ret` | x live across back-edge |
| 17 | `stage15_35_compute_liveness_dead_after_def_no_read` | `bb0: x=y; y=x; ret` | Def+Use in same block, no successor read → empty live_out |
| 18 | `stage15_35_compute_liveness_call_destination_def` | `bb0: call f() → x (self-loop)` | Call destination is a Def |
| 19 | `stage15_35_compute_liveness_empty_body` | `bb0: unreachable` | Empty body, no panic |
| 20 | `stage15_35_compute_liveness_returns_total_map` | 3-block chain | Every block has an entry in both maps |
| 21 | `stage15_35_compute_liveness_unused_local_with_mutability` | `let mut x = 1; ret` | Mutability doesn't affect liveness |

## 3. Integration Test Module

**Path**: `tests/v0/stage15/plan/nll_fixpoint_liveness_tests.rs`
**Registered as**: `stage15_nll_fixpoint_liveness_tests` (in `tests/all_tests.rs`)

### 3.1 Part A — Synthetic CFG coverage (8 tests)

Precise assertions on `LiveIn` / `LiveOut` sets for hand-built CFGs.

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_35_integration_straight_line_x_live_y_dead` | x read in `y = x` → x ∈ live_in[bb0]; y written but never read → y ∉ live_in[bb0] |
| 2 | `stage15_35_integration_branch_x_live_in_all_blocks` | Branch where x is used in both arms: x live in bb0, bb1, bb2 |
| 3 | `stage15_35_integration_loop_x_live_across_iterations` | Loop with back-edge: x live_in[bb1], live_out[bb2], live_in[bb2], live_out[bb1], live_out[bb0]; NOT live_in[bb0] (because bb0 writes x first) |
| 4 | `stage15_35_integration_drop_no_unwind_one_succ` | Drop without unwind has exactly one successor |
| 5 | `stage15_35_integration_call_with_target_one_succ` | Non-divergent Call has exactly one successor |
| 6 | `stage15_35_integration_call_no_target_zero_succ` | Divergent Call (target=None) has zero successors |
| 7 | `stage15_35_integration_total_map_all_blocks_present` | Every block (including unreachable) has an entry in both maps |
| 8 | `stage15_35_integration_switchint_duplicate_targets_idempotent` | SwitchInt with duplicate targets doesn't break the union |

### 3.2 Part B — Real-pipeline smoke tests (5 tests)

Compile real Landin source via `compile()`, then call `compute_liveness`
on the resulting MIR body. The smoke tests don't assert on specific
locals (that would couple the tests to MIR lower internals) — they
verify the API accepts whatever MIR the compiler produces and converges
without panicking.

| # | Test name | Source pattern | Verifies |
|---|-----------|----------------|----------|
| 9 | `stage15_35_integration_smoke_test_real_mir` | `let x = 1; let y = 2; x + y` | Simple straight-line MIR works |
| 10 | `stage15_35_integration_smoke_test_control_flow` | nested `if/else if/else` | Multiple SwitchInt terminators work |
| 11 | `stage15_35_integration_smoke_test_loop` | `while` loop with mutation | Back-edge from loop body to header works |
| 12 | `stage15_35_integration_smoke_test_borrows` | `let r = &x; *r + 0` | `Rvalue::Ref` is handled by `rvalue_reads` |
| 13 | `stage15_35_integration_mutability_doesnt_affect_liveness` | `let mut x = 42; ret` | `Mutability::Mutable` doesn't artificially inflate liveness |

## 4. Regression Test Strategy

### 4.1 No regression expected

Stage 15.35 adds new code only — it does not modify the existing borrow
checker walk. The legacy `compute_last_use_map` is still the active
analysis. All 173 lib tests + 2013 integration tests + 5216 conformance
tests must pass unchanged.

### 4.2 Conformance tests

All 5216 conformance tests must continue to pass. The fixpoint liveness
analysis is not invoked by the borrow checker in this stage — it's
only invoked by the new tests. So conformance behavior is identical
to v0.160.0.

### 4.3 Public API additions

The new public symbols (`compute_liveness`, `successors`, `LiveInMap`,
`LiveOutMap`) are added to `borrowck` module's re-export list. No
existing public symbols are removed or renamed — only additions.

## 5. Coverage Matrix

| Feature | Unit tests | Integration tests | Total |
|---------|-----------|-------------------|-------|
| `successors` enumeration (all 7 TerminatorKind variants) | 9 | 3 (Drop, Call×2) | 12 |
| `statement_writes` | 1 | 0 (covered by compute_liveness tests) | 1 |
| `terminator_writes` | 2 | 0 (covered by compute_liveness tests) | 2 |
| `compute_liveness` straight-line | 4 | 1 | 5 |
| `compute_liveness` branch | 1 | 1 | 2 |
| `compute_liveness` loop (back-edge) | 1 | 1 | 2 |
| `compute_liveness` edge cases (empty body, total map, mutability) | 3 | 2 | 5 |
| Real-pipeline smoke (4 source patterns) | 0 | 5 | 5 |
| **Total** | **21** | **13** | **34** |

## 6. Negative Test Coverage (§9.1.1)

Stage 15.35 doesn't introduce user-facing error messages — the fixpoint
analysis is a pure function that returns liveness maps. There are no
"error paths" to test in the traditional sense. However, the tests do
cover the following negative scenarios:

| Scenario | Test |
|----------|------|
| Empty body (no statements, unreachable terminator) | `stage15_35_compute_liveness_empty_body` |
| Block with no successors (Return / Unreachable) | `stage15_35_successors_return_empty`, `stage15_35_successors_unreachable_empty` |
| Divergent Call (target=None) | `stage15_35_successors_call_no_target_divergent`, `stage15_35_integration_call_no_target_zero_succ` |
| SwitchInt with duplicate targets | `stage15_35_integration_switchint_duplicate_targets_idempotent` |
| Local written but never read | `stage15_35_compute_liveness_straight_line_dead`, `stage15_35_integration_straight_line_x_live_y_dead` |

## 7. Test Execution

```bash
# Run only the new unit tests
cargo test --features llvm-backend --lib borrowck::liveness::

# Run only the new integration tests
cargo test --features llvm-backend --test all_tests stage15_nll_fixpoint_liveness_tests

# Run all tests (regression check)
cargo test --features llvm-backend

# Run conformance tests
python3 tests/conformance/run_all.py
```

## 8. Expected Results

- **Unit tests**: 21/21 PASS
- **Integration tests**: 13/13 PASS
- **Lib tests**: 173/173 PASS (no regression)
- **Existing integration tests**: 2013/2013 PASS (no regression)
- **Conformance tests**: 5216/5216 PASS (no regression)
- **Clippy**: 0 warnings
- **Fmt**: clean

## 9. Stage Gate Review — Test Coverage (§29.1.3 Design-Impl-Test)

| Design point (from `23-nll-fixpoint.md`) | Implementation | Test |
|------------------------------------------|----------------|------|
| `LiveInMap` / `LiveOutMap` types | `src/borrowck/liveness.rs` | `stage15_35_compute_liveness_returns_total_map` |
| `compute_liveness` fixpoint | `src/borrowck/liveness.rs` | All 9 unit + 8 integration tests |
| `successors` enumeration | `src/borrowck/liveness.rs` | 9 unit + 3 integration tests |
| Backward-compat with `compute_last_use_map` | `src/borrowck/mod.rs` re-export | All existing tests pass (regression check) |
| Migration step 1 of 4 (15.34→15.37) | This stage adds API; wiring in 15.36-15.37 | N/A (no borrowck behavior change) |
| §23 API naming compliance | All new symbols follow conventions | Manual review (passed) |

All design points have implementation and tests. No "design requires but
not implemented" or "implemented but not tested" gaps.
