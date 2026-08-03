# Stage 15.98 — Test Plan: Region Inference All-Pairs Matching

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.222.0 → v0.223.0

## 1. Test Scope

Stage 15.98 replaces 3 "first-to-first" region matching sites with
all-pairs matching. No new tests — correctness verified by all existing
tests passing (no false positives from the tighter constraints).

## 2. Acceptance Criteria

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build` | 0 warnings | ✅ |
| `cargo fmt` | clean | ✅ |
| `cargo clippy` | 0 warnings | ✅ |
| `cargo test --lib` | 244/244 | ✅ |
| `cargo test --test all_tests` | 2144/2144 | ✅ |
| conformance | 5224/5224 | ✅ |

**Stage 15.98 PASSED**.
