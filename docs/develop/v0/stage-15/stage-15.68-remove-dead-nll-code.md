# Stage 15.68 — Remove Dead NLL Code

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.193.0 → v0.194.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 5 "去除兼容思维" + §15 "最优 > 最小"

## 1. Executive Summary

Stage 15.68 removes dead code left over from the GAP-1 compromise (Option B).
After Stage 15.67 implemented true Rust NLL (liveness-based kill), the
following functions and types were no longer used by the borrow checker:

- `compute_last_use_map` — the legacy single-pass last-use analysis.
- `compute_ever_read` — the "was ever read" set (Option B's GAP-1 guard).
- `LastUseMap` type alias.

Per §1.0 原則 5 "去除兼容思维" and §15 "最优 > 最小", dead code is removed.

**Key results**:
- Removed `compute_last_use_map` function (~20 lines) + `LastUseMap` type alias.
- Removed `compute_ever_read` function (~15 lines) + its 5 unit tests.
- Removed re-exports from `borrowck/mod.rs`.
- Updated 2 integration test files to remove tests for removed functions.
- All 7567 tests pass (221 lib + 2130 integration + 5216 conformance).

## 2. What Was Removed

### 2.1 `src/borrowck/liveness.rs`
- `pub type LastUseMap` — type alias removed.
- `pub fn compute_last_use_map` — function removed.
- `pub fn compute_ever_read` — function removed.
- 5 `compute_ever_read` unit tests removed.

### 2.2 `src/borrowck/mod.rs`
- Removed `compute_ever_read`, `compute_last_use_map`, `LastUseMap` from re-exports.

### 2.3 Test files updated
- `tests/v0/stage15/plan/option_b_implementation_tests.rs` — removed `compute_ever_read` import + 2 API tests.
- `tests/v0/stage15/plan/stage15_41_legacy_delegation_tests.rs` — removed `compute_last_use_map` import + 1 test.

## 3. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 221/221 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2130/2130 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS

**Total: 7567 tests passing, 0 failures, 0 warnings.**

## 4. Committee Vote: GO

**Decision**: Stage 15.68 is **COMPLETE**. Dead NLL code removed per
§1.0 原則 5 "去除兼容思维" and §15 "最优 > 最小".
