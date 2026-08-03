# Stage 15.74 — Remove Duplicate Copy Detection (DRY)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.198.0 → v0.199.0
> **Process**: stage-committee-process.md v3.24 §23 rule 5 (DRY)

## 1. Executive Summary

Stage 15.74 removes the duplicate `is_capture_ty_copy` function from
`src/mir/lower/expr_operand.rs`. This function was an inline copy of
`is_mir_ty_copy_conservative` from `src/mir/ty.rs` (added in Stage 15.64).
The closure capture code now uses the shared helper directly.

Per §23 rule 5 (DRY): single source of truth for conservative Copy detection.
Per §1.0 原則 5 "去除兼容思维": duplicate code removed.

**Total: 7567 tests passing, 0 failures, 0 warnings.**

## 2. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 221/221 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2130/2130 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
