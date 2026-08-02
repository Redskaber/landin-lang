# Stage 15.71 — Test Plan: fn_sigs Region Inference

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.195.0 → v0.196.0

## 1. Test Categories

### 1.1 Regression — All 5216 conformance tests pass
No regression. The fn_sigs integration is backward compatible (resolver
NOT passed, unsound `ty_is_copy` retained).

### 1.2 Integration — 2130 tests pass
All integration tests pass including region inference tests.

## 2. Sign-off
- ✅ All 5216 conformance tests pass.
- ✅ All 2130 integration tests pass.
- ✅ All 221 lib tests pass.
- ✅ 0 clippy warnings, fmt clean.

**Total: 7567 tests passing, 0 failures.**

Stage 15.71 is GO for merge.
