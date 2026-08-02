# Stage 15.67 — Test Plan: True Rust NLL

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.192.0 → v0.193.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Categories

### 1.1 Conformance — 108 tests flipped to compile_ok
108 GAP-1 tests flipped from `compile_error` to `compile_ok`. These are valid
NLL programs (e.g., `let r1 = &mut x; let r2 = &mut x;` where r1 never read).

### 1.2 Regression — All 5216 conformance tests pass
No regressions — the state-machine test (original false positive case) now
passes.

### 1.3 Integration tests — 7 tests updated
Tests that asserted GAP-1 rejection now assert true NLL acceptance.

### 1.4 Lib tests — 2 tests updated
`move_borrowed_detected` and `assign_to_borrowed_detected` now assert no
errors (true NLL allows never-read borrows to expire).

## 2. Sign-off
- ✅ All 5216 conformance tests pass.
- ✅ All 2133 integration tests pass.
- ✅ All 226 lib tests pass.
- ✅ 0 clippy warnings, fmt clean.
- ✅ Runtime: state-machine test passes (false positive fixed).

**Total: 7575 tests passing, 0 failures.**

Stage 15.67 is GO for merge.
