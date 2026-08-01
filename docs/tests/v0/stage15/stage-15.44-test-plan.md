# Stage 15.44 — Test Plan: `elaborate_drops` Pass

> **Date**: 2026-08-01
> **Version**: v0.169.0 → v0.170.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.44-elaborate-drops.md`

## 1. Test Scope

| Area | Test type | Count |
|------|-----------|-------|
| `elaborate_drops` no-op when no drop needed | Unit | 2 |
| `elaborate_drops` on real MIR (no-op) | Integration | 3 |
| **Total new** | | **5** |
| Regression | All | 224 lib + 2079 integration + 5216 conformance |

## 2. Expected Results

- **Unit tests**: 18/18 PASS (16 from 15.43 + 2 new)
- **Integration tests**: 3/3 PASS
- **Lib tests**: 226/226 PASS
- **Integration tests**: 2082/2082 PASS
- **Conformance**: 5216/5216 PASS
- **Clippy**: 0 warnings
- **Fmt**: clean
