# Stage 15.72 — Remove Deprecated Borrowck Code

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.196.0 → v0.197.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 5 "去除兼容思维"

## 1. Executive Summary

Stage 15.72 removes all deprecated borrow checker code:
- `BorrowChecker::check_mir_body` (deprecated method alias).
- `check_mir_body` free function (deprecated convenience wrapper).
- `check_crate` free function (deprecated §16-violating HIR re-lowering).
- `#[allow(deprecated)]` attributes in test files.
- Updated all 14 test files to use `check_mir_body_with_dataflow` directly.

Per §1.0 原則 5 "去除兼容思维" and §15 "最优 > 最小": dead/deprecated code removed.

**Total: 7567 tests passing, 0 failures, 0 warnings.**

## 2. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 221/221 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2130/2130 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
