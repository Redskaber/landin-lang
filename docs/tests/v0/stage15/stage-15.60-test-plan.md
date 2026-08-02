# Stage 15.60 — Test Plan: DefId Fix

> **Date**: 2026-08-01
> **Version**: v0.185.0 → v0.186.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3

## 1. Test Scope

| Area | Test type | Count |
|------|-----------|-------|
| Regression (existing tests) | All | 226 lib + 2094 integration + 5216 conformance |

## 2. Expected Results

- **Lib tests**: 226/226 PASS
- **Conformance tests**: 5216/5216 PASS
- **Clippy**: 0 warnings
- **Fmt**: clean
