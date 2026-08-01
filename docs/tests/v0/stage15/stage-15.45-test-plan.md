# Stage 15.45 — Test Plan: Drop Glue Codegen

> **Date**: 2026-08-01
> **Version**: v0.170.0 → v0.171.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.45-drop-glue-codegen.md`

## 1. Test Scope

Stage 15.45 modifies `TerminatorKind::Drop` codegen to be non-noop.
No new tests are added because the code path is not exercised (no `Drop`
terminators are generated until `impl Drop` support is added in Stage
15.46). All existing tests verify zero regression.

| Area | Test type | Count |
|------|-----------|-------|
| Regression (existing tests) | All | 226 lib + 2082 integration + 5216 conformance |

## 2. Expected Results

- **Lib tests**: 226/226 PASS
- **Integration tests**: 2082/2082 PASS
- **Conformance tests**: 5216/5216 PASS
- **Clippy**: 0 warnings
- **Fmt**: clean
