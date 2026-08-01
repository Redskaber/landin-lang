# Stage 15.39 — Test Plan: Option B Implementation

> **Date**: 2026-08-01
> **Version**: v0.164.0 → v0.165.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3
> **Design doc**: `docs/lang-design/24-gap1-reconciliation.md`
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.39-option-b-implementation.md`

## 1. Test Scope

Stage 15.39 implements Option B from the GAP-1 reconciliation design
doc: a `compute_ever_read` pre-pass + modified `kill_expired_borrows_dataflow`
that preserves GAP-1 semantics in the dataflow borrow-check path.

This test plan validates:
1. **`compute_ever_read` correctness** — the function correctly collects
   all locals read anywhere in the MIR body.
2. **GAP-1 preservation** — the dataflow path now rejects the same GAP-1
   patterns the legacy path rejects (the main goal of Option B).
3. **Loop-borrow soundness preserved** — the dataflow path still
   correctly handles loop-carried borrows.
4. **Parity on valid programs** — no regression on valid code.
5. **Known limitation documented** — the `&mut self` false positive is
   documented with a test that will be updated when the fix lands.

| Area | Test type | Count |
|------|-----------|-------|
| `compute_ever_read` correctness (5 cases) | Unit | 5 |
| GAP-1 preservation (3 patterns) | Integration | 3 |
| Loop-borrow soundness | Integration | 1 |
| Parity on valid programs | Integration | 2 |
| `compute_ever_read` public API | Integration | 2 |
| Known limitation documentation | Integration | 1 |
| **Total new** | | **14** |
| Regression (existing tests) | All | 173 lib + 2052 integration + 5216 conformance |

## 2. Unit Test Module

**Path**: `src/borrowck/liveness.rs` (inline `mod tests`)
**Coverage**: `compute_ever_read` algorithm internals.

### 2.1 `compute_ever_read` tests (5)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_39_compute_ever_read_collects_all_reads` | Collects locals read in statements (x and y in `y = x; z = y;`) |
| 2 | `stage15_39_compute_ever_read_includes_terminator_reads` | Includes locals read by terminators (SwitchInt discr) |
| 3 | `stage15_39_compute_ever_read_empty_when_no_reads` | Empty set when only writes (no reads) |
| 4 | `stage15_39_compute_ever_read_empty_body` | Empty set for empty body (no panic) |
| 5 | `stage15_39_compute_ever_read_multiple_blocks` | Collects reads across multiple basic blocks |

## 3. Integration Test Module

**Path**: `tests/v0/stage15/plan/option_b_implementation_tests.rs`
**Registered as**: `stage15_option_b_implementation_tests` (in `tests/all_tests.rs`)
**Module attribute**: `#![allow(deprecated)]` — calls both `check_mir_body`
(deprecated) and `check_mir_body_with_dataflow` for comparison.

### 3.1 Part A — GAP-1 preservation (3 tests, the main goal of Option B)

| # | Test name | Pattern | Verifies |
|---|-----------|---------|----------|
| 1 | `stage15_39_option_b_preserves_gap1_double_mut_borrow` | `let r1 = &mut x; let r2 = &mut x;` | Both paths reject (GAP-1 preserved) |
| 2 | `stage15_39_option_b_preserves_gap1_shared_then_mut` | `let r = &x; let r2 = &mut x;` | Both paths reject (GAP-1 preserved) |
| 3 | `stage15_39_option_b_preserves_gap1_borrow_then_mutate_after_scope` | `{ let r = &x; } x = 2;` | Both paths reject (GAP-1 preserved) |

**These are the key acceptance tests for Stage 15.39.** Before Option B,
the dataflow path accepted these patterns (GAP-1 conflict). After Option B,
the dataflow path rejects them, matching the legacy path.

### 3.2 Part B — Loop-borrow soundness (1 test, preserved from Stage 15.36)

| # | Test name | Pattern | Verifies |
|---|-----------|---------|----------|
| 4 | `stage15_39_option_b_preserves_loop_borrow_soundness` | `let r = &x; while i < 3 { s += *r; }` | Dataflow path accepts (soundness preserved) |

This test verifies that Option B didn't break the loop-borrow soundness
improvement from Stage 15.36. The `r` IS read (inside the loop), so
it's in `ever_read`, and the normal NLL liveness check applies — `r` is
live across the loop, so its borrow survives.

### 3.3 Part C — Parity on valid programs (2 tests)

| # | Test name | Pattern | Verifies |
|---|-----------|---------|----------|
| 5 | `stage15_39_option_b_parity_valid_program` | `let x = 1; let y = 2; x + y` | Both paths produce 0 errors |
| 6 | `stage15_39_option_b_parity_single_borrow` | `let r = &x; *r` | Both paths produce 0 errors |

### 3.4 Part D — `compute_ever_read` public API (2 tests)

| # | Test name | Verifies |
|---|-----------|----------|
| 7 | `stage15_39_compute_ever_read_callable_on_real_mir` | Callable on real MIR, no panic |
| 8 | `stage15_39_compute_ever_read_empty_for_no_reads` | Returns empty/small set for no-read program |

### 3.5 Part E — Known limitation (1 test, documents the false positive)

| # | Test name | Verifies |
|---|-----------|----------|
| 9 | `stage15_39_known_limitation_mut_self_method_call_in_loop` | Documents the `&mut self` false positive — legacy accepts, dataflow rejects. Will be updated when the fix lands. |

**This test documents the known limitation.** It verifies that the legacy
path accepts the pattern (no regression) and logs the dataflow path's
false positive. When the false positive is fixed, the test should be
updated to assert `dataflow_errors.is_empty()`.

## 4. Regression Test Strategy

### 4.1 Conformance tests (5216) — zero regression expected

The driver still uses the legacy `check_mir_body` (driver switch is
blocked by the 1 false positive). All 5216 conformance tests must pass
unchanged.

### 4.2 Existing integration tests (2052) — zero regression expected

The Stage 15.36 tests (`kill_borrows_dataflow_tests.rs`) and Stage 15.37
tests (`stage15_37_driver_switch_tests.rs`) continue to pass — the
dataflow path's behavior on their test cases is unchanged (those tests
don't involve GAP-1 patterns).

### 4.3 Diagnostic tool re-run — LEGACY-STRICTER must be 0

The Stage 15.38 diagnostic tool is re-run after Option B. The
LEGACY-STRICTER count must be 0 (was 112 before Option B). This is the
key acceptance criterion:

```
Files scanned: 5216 (skipped: 188)
Files compared: 5028
  AGREE-OK:           4829
  AGREE-ERROR:        198   (was 86 — the 112 GAP-1 cases moved here)
  LEGACY-STRICTER:    0     (was 112 — RESOLVED)
  DATAFLOW-STRICTER:  1     (unchanged — known limitation)
  DIFFERENT-ERRORS:   0
```

## 5. Coverage Matrix

| Feature | Unit tests | Integration tests | Total |
|---------|-----------|-------------------|-------|
| `compute_ever_read` correctness (5 cases) | 5 | 2 (API smoke) | 7 |
| GAP-1 preservation (3 patterns) | 0 | 3 | 3 |
| Loop-borrow soundness | 0 | 1 | 1 |
| Parity on valid programs | 0 | 2 | 2 |
| Known limitation documentation | 0 | 1 | 1 |
| **Total** | **5** | **9** | **14** |

## 6. Negative Test Coverage (§9.1.1)

| Scenario | Test |
|----------|------|
| GAP-1 double-mut-borrow (must reject) | `stage15_39_option_b_preserves_gap1_double_mut_borrow` |
| GAP-1 shared-then-mut (must reject) | `stage15_39_option_b_preserves_gap1_shared_then_mut` |
| GAP-1 borrow-then-mutate-after-scope (must reject) | `stage15_39_option_b_preserves_gap1_borrow_then_mutate_after_scope` |
| Empty body (no panic) | `stage15_39_compute_ever_read_empty_body` |
| No reads (empty set) | `stage15_39_compute_ever_read_empty_when_no_reads` |
| `&mut self` false positive (known limitation) | `stage15_39_known_limitation_mut_self_method_call_in_loop` |

## 7. Test Execution

```bash
# Run only the new unit tests
cargo test --features llvm-backend --lib borrowck::liveness::tests::stage15_39

# Run only the new integration tests
cargo test --features llvm-backend --test all_tests stage15_option_b_implementation_tests

# Re-run the diagnostic tool to verify LEGACY-STRICTER is 0
cargo test --features llvm-backend --test all_tests stage15_borrowck_comparison_diagnostic -- --nocapture

# Run all tests (regression check)
cargo test --features llvm-backend

# Run conformance tests
python3 tests/conformance/run_all.py
```

## 8. Expected Results

- **Unit tests**: 5/5 PASS
- **Integration tests**: 9/9 PASS
- **Lib tests**: 178/178 PASS (173 + 5 new, zero regression)
- **Existing integration tests**: 2052/2052 PASS (zero regression)
- **Conformance tests**: 5216/5216 PASS (zero regression)
- **Diagnostic tool**: LEGACY-STRICTER = 0 (was 112)
- **Clippy**: 0 warnings
- **Fmt**: clean

## 9. Stage Gate Review — Test Coverage (§29.1.3 Design-Impl-Test)

| Design point (from `24-gap1-reconciliation.md` §5) | Implementation | Test |
|-----------------------------------------------------|----------------|------|
| `compute_ever_read` helper | `src/borrowck/liveness.rs` | 5 unit + 2 integration tests |
| Modified `kill_expired_borrows_dataflow` with `ever_read` param | `src/borrowck/mod.rs` | GAP-1 preservation tests (3) |
| `check_mir_body_with_dataflow` computes `ever_read` | `src/borrowck/mod.rs` | Loop-borrow soundness test (1) |
| GAP-1 preserved (112 → 0) | Diagnostic tool re-run | LEGACY-STRICTER = 0 |
| Loop-borrow soundness preserved | Stage 15.36 tests still pass | `stage15_39_option_b_preserves_loop_borrow_soundness` |
| Known limitation documented | `&mut self` false positive test | `stage15_39_known_limitation_mut_self_method_call_in_loop` |
| §23 API naming compliance | All new symbols follow conventions | Manual review (passed) |

All design points have implementation and tests. No "design requires but
not implemented" or "implemented but not tested" gaps.
