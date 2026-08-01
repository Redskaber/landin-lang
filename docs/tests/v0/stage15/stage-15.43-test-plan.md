# Stage 15.43 — Test Plan: `ty_needs_drop` Analysis

> **Date**: 2026-08-01
> **Version**: v0.168.0 → v0.169.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.43-ty-needs-drop.md`

## 1. Test Scope

| Area | Test type | Count |
|------|-----------|-------|
| `ty_needs_drop` on all TyKind variants | Unit | 16 |
| `ty_needs_drop` on real MIR | Integration | 3 |
| **Total new** | | **19** |
| Regression | All | 208 lib + 2076 integration + 5216 conformance |

## 2. Expected Results

- **Unit tests**: 16/16 PASS
- **Integration tests**: 3/3 PASS
- **Lib tests**: 224/224 PASS (208 + 16 new)
- **Integration tests**: 2079/2079 PASS (2076 + 3 new)
- **Conformance**: 5216/5216 PASS
- **Clippy**: 0 warnings
- **Fmt**: clean
