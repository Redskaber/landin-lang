# Stage 15.58 — Test Plan: impl Drop Conformance + Integration Tests

> **Date**: 2026-08-01
> **Version**: v0.183.0 → v0.184.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3

## 1. Test Scope

| Area | Test type | Count |
|------|-----------|-------|
| No-Drop programs compile cleanly (no false positives) | Integration | 3 |
| Regression (existing tests) | All | 226 lib + 2091 integration + 5216 conformance |

## 2. Expected Results

- **Lib tests**: 226/226 PASS
- **Integration tests**: 2094/2094 PASS (2091 + 3 new)
- **Conformance tests**: 5216/5216 PASS
- **Clippy**: 0 warnings
- **Fmt**: clean
