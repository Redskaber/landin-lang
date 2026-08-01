# Stage 15.37 — Test Plan: Driver Switch (DEFERRED) + Legacy Deprecation + GAP-1 Conflict

> **Date**: 2026-08-01
> **Version**: v0.162.0 → v0.163.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3
> **Design doc**: `docs/lang-design/23-nll-fixpoint.md`
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.37-driver-switch-and-legacy-removal.md`

## 1. Test Scope

Stage 15.37 was planned as the final step of the NLL fixpoint migration:
switch the driver to `check_mir_body_with_dataflow`, deprecate the
legacy `check_mir_body`, and remove `compute_last_use_map` +
`kill_expired_borrows` (dead code).

**The driver switch was attempted, regressed 112 conformance tests, and
was reverted.** The regression is due to a semantic conflict between
the dataflow path (correct NLL) and the Stage 14.81 GAP-1 soundness
fix (stricter semantics that 112 conformance tests depend on).

This test plan validates:
1. **Deprecation API contract** — `check_mir_body` is marked
   `#[deprecated]` with a note pointing to
   `check_mir_body_with_dataflow`. The deprecated API still works
   (deprecation is a warning, not removal).
2. **Driver behavior unchanged** — the driver still uses the legacy
   `check_mir_body` (dataflow switch deferred). All conformance tests
   pass identically to v0.162.0.
3. **Dataflow path still accessible** — `check_mir_body_with_dataflow`
   is callable and produces correct results on soundness patterns
   (loop-carried borrows).
4. **GAP-1 semantic conflict documented** — a regression test
   documents the case where legacy and dataflow disagree, so future
   reconciliation work has a clear acceptance criterion.
5. **Parity on non-conflict cases** — for valid programs without the
   GAP-1 pattern, legacy and dataflow agree (both produce 0 errors).

| Area | Test type | Count |
|------|-----------|-------|
| Deprecation smoke (legacy still callable) | Integration | 2 |
| Driver integration (legacy path active) | Integration | 2 |
| Dataflow path accessibility | Integration | 2 |
| GAP-1 semantic conflict documentation | Integration | 1 |
| Parity on valid programs | Integration | 2 |
| **Total new** | | **9** |
| Regression (existing tests) | All | 173 lib + 2039 integration + 5216 conformance |

## 2. Integration Test Module

**Path**: `tests/v0/stage15/plan/stage15_37_driver_switch_tests.rs`
**Registered as**: `stage15_37_driver_switch_tests` (in `tests/all_tests.rs`)
**Module attribute**: `#![allow(deprecated)]` — the tests intentionally
call the deprecated `check_mir_body` to verify it still works and to
compare against the dataflow path.

### 2.1 Part A — Deprecation smoke tests (2)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_37_legacy_check_mir_body_still_callable` | Deprecated free fn `check_mir_body` still works (produces correct results on valid programs). Per §23.1 rule 6: deprecation is a warning, not removal. |
| 2 | `stage15_37_legacy_borrow_checker_method_still_callable` | Deprecated method `BorrowChecker::check_mir_body` still works. |

### 2.2 Part B — Driver integration tests (2)

| # | Test name | Verifies |
|---|-----------|----------|
| 3 | `stage15_37_driver_uses_legacy_path_no_regression` | Driver behavior unchanged from v0.162.0 — valid programs compile cleanly. This is the key acceptance test: even though the dataflow path exists, the driver continues to use the legacy path. |
| 4 | `stage15_37_driver_preserves_gap1_soundness` | Driver still rejects `let r1 = &mut x; let r2 = &mut x;` (GAP-1 soundness fix preserved). This is the GAP-1 guarantee that 112 conformance tests depend on. |

### 2.3 Part C — Dataflow path accessibility (2)

| # | Test name | Verifies |
|---|-----------|----------|
| 5 | `stage15_37_dataflow_path_still_accessible` | `check_mir_body_with_dataflow` is callable on real MIR and produces 0 errors on valid programs. Even though the driver doesn't use it, the API must remain available for testing and future migration. |
| 6 | `stage15_37_dataflow_path_handles_loop_borrow` | Dataflow path correctly handles loop-carried borrows (the soundness pattern it was designed to fix). `let r = &x; while i < 3 { s += *r; }` must produce 0 errors. |

### 2.4 Part D — GAP-1 semantic conflict documentation (1)

| # | Test name | Verifies |
|---|-----------|----------|
| 7 | `stage15_37_gap1_semantic_conflict_documented` | **Documents the GAP-1 conflict**: for `let r1 = &mut x; let r2 = &mut x;`, the legacy path rejects (GAP-1 fix) while the dataflow path accepts (correct NLL — `r1` is dead). This test is the acceptance criterion for future reconciliation work: when the conflict is resolved, this test will need to be updated. |

### 2.5 Part E — Parity on non-conflict cases (2)

| # | Test name | Verifies |
|---|-----------|----------|
| 8 | `stage15_37_parity_on_valid_program` | For valid programs WITHOUT the GAP-1 pattern, legacy and dataflow agree (both produce 0 errors). This is the same parity criterion from Stage 15.36, re-run after the deprecation changes to verify no regression. |
| 9 | `stage15_37_parity_on_single_borrow` | Legacy and dataflow agree on single-borrow programs (no conflict possible). |

## 3. Regression Test Strategy

### 3.1 Conformance tests (5216) — zero regression expected

The driver switch was reverted, so the driver's behavior is identical
to v0.162.0. All 5216 conformance tests must pass unchanged. This was
verified after the revert:

```
Results: 5216 passed, 0 failed, 5216 total
ALL TESTS PASSED
```

### 3.2 Existing integration tests (2039) — zero regression expected

The 7 test files patched with `#![allow(deprecated)]` continue to call
the legacy `check_mir_body`. Their behavior is unchanged — the
deprecation is a warning, not a behavior change. All 2039 existing
integration tests must pass.

### 3.3 Lib tests (173) — zero regression expected

The `borrowck::mod::tests` module is wrapped in `#[allow(deprecated)]`
(added in Stage 15.37). All 173 lib tests must pass unchanged.

### 3.4 The 112 conformance tests that failed during the driver switch attempt

These tests were temporarily failing when the driver was switched to
`check_mir_body_with_dataflow`. After the revert, they pass again.
They are NOT modified in Stage 15.37 — they remain as `compile_error`
(GAP-1 semantics). Future reconciliation work (Options A/B/C in the
develop doc §4.5) will decide their fate.

## 4. Coverage Matrix

| Feature | Unit tests | Integration tests | Total |
|---------|-----------|-------------------|-------|
| Deprecation API contract (legacy still callable) | 0 | 2 | 2 |
| Driver integration (legacy path active) | 0 | 2 | 2 |
| Dataflow path accessibility | 0 | 2 | 2 |
| GAP-1 semantic conflict documentation | 0 | 1 | 1 |
| Parity on non-conflict cases | 0 | 2 | 2 |
| **Total** | **0** | **9** | **9** |

## 5. Negative Test Coverage (§9.1.1)

Stage 15.37 doesn't introduce user-facing error messages. The key
"negative" scenario tested is the GAP-1 conflict — a program that one
path rejects and the other accepts:

| Scenario | Test |
|----------|------|
| GAP-1 conflict pattern (`let r1 = &mut x; let r2 = &mut x;`) | `stage15_37_gap1_semantic_conflict_documented` — documents that legacy rejects, dataflow accepts |
| Driver preserves GAP-1 soundness | `stage15_37_driver_preserves_gap1_soundness` — driver (legacy path) rejects the pattern |

## 6. Test Execution

```bash
# Run only the new Stage 15.37 tests
cargo test --features llvm-backend --test all_tests stage15_37_driver_switch_tests

# Run all tests (regression check)
cargo test --features llvm-backend

# Run conformance tests (must be 5216/5216 — driver switch was reverted)
python3 tests/conformance/run_all.py

# Verify no deprecation warnings leak (clippy --all-targets should be clean)
cargo clippy --all-targets --features llvm-backend
```

## 7. Expected Results

- **Stage 15.37 integration tests**: 9/9 PASS
- **Lib tests**: 173/173 PASS (zero regression)
- **Existing integration tests**: 2039/2039 PASS (zero regression)
- **Conformance tests**: 5216/5216 PASS (zero regression — driver switch reverted)
- **Clippy**: 0 warnings
- **Fmt**: clean

## 8. Stage Gate Review — Test Coverage (§29.1.3 Design-Impl-Test)

| Design point | Implementation | Test |
|--------------|----------------|------|
| Legacy `check_mir_body` deprecated with note | `src/borrowck/mod.rs` (method + free fn) | `stage15_37_legacy_check_mir_body_still_callable`, `stage15_37_legacy_borrow_checker_method_still_callable` |
| Driver switch DEFERRED (GAP-1 conflict) | `src/driver.rs` (reverted, `#[allow(deprecated)]` at call site) | `stage15_37_driver_uses_legacy_path_no_regression`, `stage15_37_driver_preserves_gap1_soundness` |
| Dataflow path still accessible | `check_mir_body_with_dataflow` (Stage 15.36, unchanged) | `stage15_37_dataflow_path_still_accessible`, `stage15_37_dataflow_path_handles_loop_borrow` |
| GAP-1 semantic conflict documented | `docs/develop/v0/stage-15/stage-15.37-...md` §4 | `stage15_37_gap1_semantic_conflict_documented` |
| Parity on non-conflict cases | (No code change — Stage 15.36 parity still holds) | `stage15_37_parity_on_valid_program`, `stage15_37_parity_on_single_borrow` |
| `compute_last_use_map` + `kill_expired_borrows` retained | `src/borrowck/liveness.rs`, `src/borrowck/mod.rs` (NOT removed) | All existing tests pass (regression check) |
| Test files patched with `#[allow(deprecated)]` | 7 existing test files + 2 new test files | All pass with 0 deprecation warnings |

All design points have implementation and tests. The deferral is
documented with a clear acceptance criterion (the GAP-1 conflict
regression test) for future reconciliation work.
