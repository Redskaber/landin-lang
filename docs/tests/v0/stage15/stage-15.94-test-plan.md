# Stage 15.94 — Test Plan: Lifetime Elision + Region Inference Conformance Tests

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.218.0 → v0.219.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.94 adds 8 conformance tests for lifetime elision rules and
region inference, verifying end-to-end compilation of the features
implemented in Stages 15.90-15.93.

## 2. New Conformance Tests (8 tests)

All tests are in `tests/conformance/01-typecheck/04-lifetimes/` and
expect `compile_ok`:

| # | File | Rule |
|---|------|------|
| 1 | elision-rule-2-single-input.lin | Rule 2: single input → output |
| 2 | elision-rule-3-self-param.lin | Rule 3: &self → output |
| 3 | elision-rule-3-self-with-arg.lin | Rule 3: &self + arg |
| 4 | explicit-lifetime-dedup.lin | Explicit dedup |
| 5 | elision-no-output-ref.lin | No output ref |
| 6 | elision-tuple-return.lin | Tuple return with ref |
| 7 | elision-nested-ref.lin | Chained refs |
| 8 | explicit-multi-lifetime.lin | Multiple lifetimes |

## 3. Acceptance Criteria

| Criterion | Target | Actual |
|-----------|--------|--------|
| conformance tests | 5224 (was 5216, +8) | ✅ 5224 |
| `cargo test --lib` | 244/244 | ✅ |
| `cargo test --test all_tests` | 2144/2144 | ✅ |

**Stage 15.94 PASSED**.
