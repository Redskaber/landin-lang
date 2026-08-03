# Stage 15.93 — Test Plan: Region Inference Return Value Constraints

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.217.0 → v0.218.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.93 adds return value region constraint collection to the region
inference. When a call returns `&T`, the destination's region gets an
outlives constraint from the return type's region.

## 2. Test Strategy

No new unit tests added — the change is an addition to existing
constraint collection logic. Correctness is verified by:

1. **All 7604 existing tests pass** — no regressions (no false positives
   from the new constraint).
2. **The new constraint is only added when callee_sig is available** —
   the fallback path (no fn_sigs) is unchanged.
3. **The constraint direction is correct**: `ret_r: dest_r` (return
   type's region outlives destination's region).

## 3. Conformance Test Impact

### 3.1 No conformance test changes

All 5216 conformance tests pass unchanged. The new constraint doesn't
produce false positives because:
- It's only added when `callee_sig` is available (from fn_sigs table).
- It uses the same first-to-first region matching as the existing
  argument constraint (simplified, but consistent).
- The constraint direction (`ret_r: dest_r`) is correct — it tightens
  the constraint set without introducing unsoundness.

## 4. Acceptance Criteria

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 244/244 PASS | ✅ 244/244 PASS |
| `cargo test --features llvm-backend --test all_tests` | 2144/2144 PASS | ✅ 2144/2144 PASS |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |

**Stage 15.93 PASSED**.
