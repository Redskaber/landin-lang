# Stage 15.89 — Test Plan: Trait Error Span Accuracy Fix

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.213.0 → v0.214.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.89 fixes the last `Span::DUMMY` error category: trait errors.
Adds `span` field to `ImplInfo`, `CoherenceError`, and `IncompleteImpl`;
populates from HIR; uses in `to_diagnostics`.

## 2. New Integration Tests (2 tests)

Added to `tests/v0/stage15/plan/error_system_cleanup_tests.rs`:

### 2.1 `stage15_89_coherence_error_span_points_to_impl`

Tests that conflicting impls (`impl T for S {} impl T for S {}`) produce
a trait error whose span points to the first impl block (byte offset >= 21),
not `1:1`.

### 2.2 `stage15_89_incomplete_impl_error_span_points_to_impl`

Tests that an incomplete impl (`impl T for S {}` where T requires method
`f`) produces a trait error whose span points to the impl block (byte
offset >= 35), not `1:1`.

## 3. Conformance Test Impact

### 3.1 No conformance test changes

All 5216 conformance tests pass unchanged.

## 4. Manual Verification

### 4.1 Conflicting impls — span points to first impl

```
$ echo 'trait T {} struct S; impl T for S {} impl T for S {} fn main() {}' | landin-stage0 --compile
error[E600]: conflicting implementations of trait `T` for type `S` (2 impl blocks)
  --> /tmp/t.lin:1:22
  |
1 | trait T {} struct S; impl T for S {} impl T for S {} fn main() {}
  |                      ^^^^
```

### 4.2 Incomplete impl — span points to impl block

```
$ echo 'trait T { fn f(&self); } struct S; impl T for S {} fn main() {}' | landin-stage0 --compile
error[E600]: impl `T` for `S` is missing method(s): f
  --> /tmp/t2.lin:1:36
  |
1 | trait T { fn f(&self); } struct S; impl T for S {} fn main() {}
  |                                    ^^^^
```

## 5. Acceptance Criteria

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 236/236 PASS | ✅ 236/236 PASS |
| `cargo test --features llvm-backend --test all_tests` | 2144/2144 PASS | ✅ 2144/2144 PASS (was 2142, +2 new) |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |

**Stage 15.89 PASSED**.
