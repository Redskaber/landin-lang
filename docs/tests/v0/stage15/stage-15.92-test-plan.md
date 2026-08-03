# Stage 15.92 — Test Plan: Explicit Lifetime Tracking

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.216.0 → v0.217.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.92 adds `lower_hir_ty_to_mir_ty_with_lifetimes` for explicit
lifetime deduplication. References with the same lifetime name share
the same RegionVid.

## 2. New Unit Tests (2 tests)

### 2.1 `explicit_lifetime_deduplication`

Tests that two `&'a i32` references with the same lifetime name share
the same RegionVid.

### 2.2 `elided_lifetime_no_deduplication`

Tests that two `&i32` references with elided lifetimes get different
RegionVids (each gets its own fresh vid per elision rule 1).

## 3. Acceptance Criteria

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 244/244 PASS | ✅ 244/244 PASS (was 242, +2 new) |
| `cargo test --features llvm-backend --test all_tests` | 2144/2144 PASS | ✅ 2144/2144 PASS |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |

**Stage 15.92 PASSED**.
