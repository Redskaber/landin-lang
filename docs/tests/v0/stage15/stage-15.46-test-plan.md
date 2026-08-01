# Stage 15.46 — Test Plan: Drop Elaboration Integration

> **Date**: 2026-08-01
> **Version**: v0.171.0 → v0.172.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.46-drop-elaboration-integration.md`

## 1. Test Scope

| Area | Test type | Count |
|------|-----------|-------|
| Driver pipeline runs elaborate_drops (no regression) | Integration | 3 |
| **Total new** | | **3** |
| Regression | All | 226 lib + 2082 integration + 5216 conformance |

## 2. Expected Results

- **Lib tests**: 226/226 PASS
- **Integration tests**: 2085/2085 PASS (2082 + 3 new)
- **Conformance tests**: 5216/5216 PASS
- **Clippy**: 0 warnings
- **Fmt**: clean
