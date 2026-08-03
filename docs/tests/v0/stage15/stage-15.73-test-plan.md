# Stage 15.73 — Test Plan: Type Propagation + Move-of-Copy Fix

> **Version**: v0.197.0 → v0.198.0

## 1. Test Categories

### 1.1 Regression — All 5216 conformance tests pass
4 tests flipped from compile_ok to compile_error (method-not-found now correctly caught).

### 1.2 Lib test updated
`use_after_move_detected` now expects no errors (i32 is Copy, Move of Copy = no-op).

## 2. Sign-off
- ✅ All 5216 conformance tests pass.
- ✅ All 2130 integration tests pass.
- ✅ All 221 lib tests pass.
- ✅ 0 clippy warnings, fmt clean.

**Total: 7567 tests passing, 0 failures.**
