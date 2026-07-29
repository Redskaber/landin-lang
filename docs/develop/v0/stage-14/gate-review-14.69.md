# Stage 14.69 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.84.0 → v0.85.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.69 fixed a user-reported dead_code warning and added string content
comparison via the `__landin_str_eq` runtime function.

## 2. Bugs Fixed

### Bug 1: `build_fn_sigs_map` dead_code warning (user-reported)

**Fix**: Added `#[cfg(feature = "llvm-backend")]` to the function.

### Bug 2: String equality was bitwise, not content comparison

**Fix**: Added `__landin_str_eq` runtime function + codegen integration.
Works for same-scope comparisons. Cross-function ABI issue is a known limitation.

## 3. Verification

- All 1951 rust tests pass (4 tests updated for new behavior)
- All 5154 conformance tests pass (was 5153, +1 new run_ok)
- 0 clippy warnings, fmt clean
- `cargo build` (without llvm-backend) → 0 warnings
- Pipeline coverage: 99.7% (691 paths, 689 verified)
