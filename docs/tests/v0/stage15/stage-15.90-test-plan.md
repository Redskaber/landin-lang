# Stage 15.90 — Test Plan: Lifetime Elision Rule 2

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.214.0 → v0.215.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.90 implements Lifetime Elision Rule 2: if a function has
exactly one input lifetime, it's assigned to all elided output lifetimes.
Adds 2 new private helpers: `collect_region_vids` and `apply_elision_rule_2`.

## 2. New Unit Tests (5 tests)

Added to `src/mir/lower/mod.rs::stage15_90_tests`:

### 2.1 `collect_region_vids_basic`
Tests that `collect_region_vids` collects the vid from a simple `&i32`
with `Region::Var(5)`.

### 2.2 `collect_region_vids_nested`
Tests that `collect_region_vids` collects vids from a tuple `&(&i32, &i32)`
with regions 1 and 2.

### 2.3 `apply_elision_rule_2_single_input`
Tests that with a single input lifetime (vid 3), the output lifetime
(vid 10) is replaced with vid 3.

### 2.4 `apply_elision_rule_2_multiple_inputs`
Tests that with multiple input lifetimes (vids 1, 2), the output lifetime
(vid 10) is NOT replaced (rule 2 doesn't apply).

### 2.5 `apply_elision_rule_2_no_inputs`
Tests that with no input lifetimes, the output lifetime (vid 10) is NOT
replaced (rule 2 doesn't apply).

## 3. Conformance Test Impact

### 3.1 No conformance test changes

All 5216 conformance tests pass unchanged. The elision rule 2 change
only affects region vid assignment in MIR, which doesn't affect
compilation success/failure for any existing test (the region inference
is still a no-op for error reporting — it just produces correct vids
now).

## 4. Acceptance Criteria

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 241/241 PASS | ✅ 241/241 PASS (was 236, +5 new) |
| `cargo test --features llvm-backend --test all_tests` | 2144/2144 PASS | ✅ 2144/2144 PASS |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |

**Stage 15.90 PASSED**.
