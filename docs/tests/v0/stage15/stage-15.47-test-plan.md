# Stage 15.47 — Test Plan: Drop Elaboration Gate Review

> **Date**: 2026-08-01
> **Version**: v0.172.0 → v0.173.0
> **Process**: stage-committee-process.md v3.23 §17 + §9.3 + §25
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.47-drop-elaboration-gate-review.md`

## 1. Test Scope

Stage 15.47 is a **review-only stage** — no code changes. The test plan
verifies that all existing tests pass (zero regression) and documents the
test coverage for Task 8.

| Area | Test type | Count |
|------|-----------|-------|
| Regression (existing tests) | All | 226 lib + 2085 integration + 5216 conformance |

## 2. Task 8 Test Coverage Summary (Stages 15.42-15.46)

| Stage | Tests | Coverage |
|-------|-------|----------|
| 15.42 | 0 (doc only) | Design doc |
| 15.43 | 16 unit + 3 integration | `ty_needs_drop` on all TyKind variants + cycle detection |
| 15.44 | 2 unit + 3 integration | `elaborate_drops` no-op + empty body + real MIR |
| 15.45 | 0 (code change, path not exercised) | Drop glue codegen (untested) |
| 15.46 | 3 integration | Driver pipeline integration (no regression) |
| **Total** | **24 new tests** | Infrastructure fully tested; actual drop path untested (deferred) |

## 3. Expected Results

- **Lib tests**: 226/226 PASS
- **Integration tests**: 2085/2085 PASS
- **Conformance tests**: 5216/5216 PASS
- **Clippy**: 0 warnings
- **Fmt**: clean
