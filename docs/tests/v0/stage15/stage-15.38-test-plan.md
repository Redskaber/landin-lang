# Stage 15.38 — Test Plan: Borrow-Check Comparison Diagnostic Tool

> **Date**: 2026-08-01
> **Version**: v0.163.0 → v0.164.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3 + §29.5
> **Design doc**: `docs/lang-design/24-gap1-reconciliation.md`
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.38-borrowck-comparison-tool.md`

## 1. Test Scope

Stage 15.38 adds a **diagnostic tool** (implemented as an integration
test) that compares the legacy borrow-check path against the dataflow
path on all 5216 conformance test files. The tool produces a categorized
report that informs the GAP-1 reconciliation decision.

This test plan validates:
1. **Tool correctness** — the categorization logic, header parser, and
   file walker work correctly.
2. **Tool completeness** — the tool scans all conformance files and
   produces a non-empty report.
3. **No regression** — the tool doesn't break any existing tests (it's
   diagnostic, not pass/fail).

| Area | Test type | Count |
|------|-----------|-------|
| `ComparisonCategory` categorization logic | Unit | 1 |
| `parse_expected` header parser | Unit | 1 |
| `discover_conformance_files` walker | Unit | 1 |
| Main diagnostic (runs on all conformance files) | Integration | 1 |
| **Total new** | | **4** |
| Regression (existing tests) | All | 173 lib + 2048 integration + 5216 conformance |

## 2. Integration Test Module

**Path**: `tests/v0/stage15/plan/borrowck_comparison_diagnostic_tests.rs`
**Registered as**: `stage15_borrowck_comparison_diagnostic_tests` (in `tests/all_tests.rs`)
**Module attribute**: `#![allow(deprecated)]` — the tool intentionally
calls both `check_mir_body` (deprecated) and `check_mir_body_with_dataflow`
to compare them.

### 2.1 Unit tests (3)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_38_comparison_category_categorization` | The `ComparisonCategory::from((legacy, dataflow))` logic correctly categorizes all 5 cases: (0,0)→AgreeOk, (1,0)→LegacyStricter, (0,1)→DataflowStricter, (2,2)→AgreeError, (1,2)→DifferentErrors. |
| 2 | `stage15_38_parse_expected_header` | The `parse_expected` function correctly parses both new format (`// EXPECTED: compile_ok`) and legacy format (`//! PASS`, `//! FAIL`), and returns `"unknown"` for files with no header. |
| 3 | `stage15_38_discover_conformance_files_finds_lin_files` | The `discover_conformance_files` walker finds non-empty `.lin` files under `tests/conformance/`, and all discovered paths end with `.lin`. |

### 2.2 Main diagnostic test (1)

| # | Test name | Verifies |
|---|-----------|----------|
| 4 | `stage15_38_borrowck_comparison_diagnostic` | **The main diagnostic test.** Discovers all `.lin` files under `tests/conformance/`, compiles each via `compile()`, runs both borrow-check paths on each MIR body, categorizes the result, writes a full report to `target/borrowck-comparison-report.txt`, prints a summary to stdout. Always passes — the report is the artifact. |

**What the test does** (detailed):

1. Calls `discover_conformance_files()` to get all `.lin` files.
2. For each file:
   - Reads the source.
   - Parses the `EXPECTED` header (for the report).
   - Calls `compile(&src)` to get `CompileResult`.
   - If no MIR is produced (parse/typeck error), skips the file.
   - For each MIR body in `result.mirs`:
     - Runs `check_mir_body(mir_body)` (legacy) → collects error count.
     - Runs `check_mir_body_with_dataflow(mir_body)` (dataflow) → collects error count.
   - Categorizes the result into one of 5 `ComparisonCategory` values.
3. Aggregates counts per category.
4. Builds a report string with:
   - Summary (files scanned, skipped, compared).
   - Category counts.
   - Full list of LEGACY-STRICTER cases (with path, expected, error count, first error message).
   - First 20 DATAFLOW-STRICTER cases.
   - First 20 DIFFERENT-ERRORS cases.
   - Conclusion (interpretation of counts).
5. Writes the report to `target/borrowck-comparison-report.txt`.
6. Prints a summary to stdout (visible with `--nocapture`).
7. Asserts that category counts sum to total compared count (sanity check).

**Why the test always passes**: The test is **diagnostic**, not pass/fail.
Its purpose is to produce the report, not to assert any specific behavior.
The report is reviewed manually by the team to inform the reconciliation
decision. The only assertions are sanity checks (files discovered, counts
sum correctly).

## 3. Regression Test Strategy

### 3.1 No regression expected

Stage 15.38 adds a new test file only — it does not modify any production
code. The driver still uses the legacy `check_mir_body`. All 173 lib
tests + 2048 integration tests + 5216 conformance tests must pass
unchanged.

### 3.2 The diagnostic test's runtime

The diagnostic test compiles 5216 `.lin` files and runs two borrow-check
paths on each. This takes ~2 seconds on the test machine. The test is
not in the critical path (it's a single test, not run on every code
change).

### 3.3 The report file

The test writes `target/borrowck-comparison-report.txt`. This file is
git-ignored (it's in `target/`). A permanent copy is saved to
`docs/develop/v0/stage-15/stage-15.38-borrowck-comparison-report.txt`
for reference.

## 4. Coverage Matrix

| Feature | Unit tests | Integration tests | Total |
|---------|-----------|-------------------|-------|
| `ComparisonCategory` categorization (5 cases) | 1 | 0 (covered by main test) | 1 |
| `parse_expected` header parser (5 cases) | 1 | 0 (covered by main test) | 1 |
| `discover_conformance_files` walker | 1 | 0 (covered by main test) | 1 |
| Main diagnostic (all 5216 conformance files) | 0 | 1 | 1 |
| **Total** | **3** | **1** | **4** |

## 5. Negative Test Coverage (§9.1.1)

Stage 15.38 doesn't introduce user-facing error messages. The "negative"
scenarios tested are:

| Scenario | Test |
|----------|------|
| File with no `EXPECTED` header | `stage15_38_parse_expected_header` returns `"unknown"` |
| Conformance root doesn't exist | (implicit — `discover_conformance_files` returns empty vec, main test asserts non-empty) |
| File can't be read | `compare_on_file` returns `None` (skipped) |
| No MIR produced (parse/typeck error) | `compare_on_file` returns `None` (skipped) |
| Out-of-memory on huge files | (not tested — conformance files are small) |

## 6. Test Execution

```bash
# Run only the Stage 15.38 diagnostic tests
cargo test --features llvm-backend --test all_tests stage15_borrowck_comparison_diagnostic

# Run with stdout visible (to see the summary)
cargo test --features llvm-backend --test all_tests stage15_borrowck_comparison_diagnostic -- --nocapture

# View the full report
cat target/borrowck-comparison-report.txt

# Run all tests (regression check)
cargo test --features llvm-backend

# Run conformance tests
python3 tests/conformance/run_all.py
```

## 7. Expected Results

- **Stage 15.38 tests**: 4/4 PASS
- **Lib tests**: 173/173 PASS (zero regression)
- **Existing integration tests**: 2048/2048 PASS (zero regression)
- **Conformance tests**: 5216/5216 PASS (zero regression)
- **Clippy**: 0 warnings
- **Fmt**: clean
- **Report**: `target/borrowck-comparison-report.txt` generated with:
  - 4829 AGREE-OK
  - 86 AGREE-ERROR
  - 112 LEGACY-STRICTER (GAP-1 conflict)
  - 1 DATAFLOW-STRICTER (false positive)
  - 0 DIFFERENT-ERRORS

## 8. Stage Gate Review — Test Coverage (§29.1.3 Design-Impl-Test)

| Design point | Implementation | Test |
|--------------|----------------|------|
| Diagnostic tool compares both paths on all conformance files | `borrowck_comparison_diagnostic_tests.rs` | `stage15_38_borrowck_comparison_diagnostic` |
| Categorizes results into 5 buckets | `ComparisonCategory` enum + `From<(usize, usize)>` impl | `stage15_38_comparison_category_categorization` |
| Parses `.lin` file headers for EXPECTED field | `parse_expected` function | `stage15_38_parse_expected_header` |
| Discovers all `.lin` files under conformance root | `discover_conformance_files` + `walk_dir` | `stage15_38_discover_conformance_files_finds_lin_files` |
| Writes a report to `target/` | `fs::write(&report_path, &report)` | (verified by running the test and checking the file) |
| Informs the reconciliation decision | `docs/lang-design/24-gap1-reconciliation.md` | (manual review — design doc references the report) |

All design points have implementation and tests. No "design requires but
not implemented" or "implemented but not tested" gaps.

## 9. Diagnostic Report Findings (for the record)

The Stage 15.38 diagnostic report (generated 2026-08-01 on v0.163.0)
found:

```
Files scanned: 5216 (skipped: 188)
Files compared: 5028
  AGREE-OK:           4829
  AGREE-ERROR:        86
  LEGACY-STRICTER:    112 (GAP-1 conflict cases)
  DATAFLOW-STRICTER:  1 (soundness improvements)
  DIFFERENT-ERRORS:   0
```

The 112 LEGACY-STRICTER cases are the GAP-1 conflict (expected). The
1 DATAFLOW-STRICTER case is a false positive on
`e2e-runok-132-state-machine.lin` (a `&mut self` method-call-heavy
program) — this was an unexpected finding that informs the
reconciliation decision.

The full report is at:
- `target/borrowck-comparison-report.txt` (generated by the test)
- `docs/develop/v0/stage-15/stage-15.38-borrowck-comparison-report.txt` (permanent copy)
