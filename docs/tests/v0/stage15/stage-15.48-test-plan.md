# Stage 15.48 — Test Plan: Region Allocation Design Doc

> **Date**: 2026-08-01
> **Version**: v0.173.0 → v0.174.0
> **Process**: stage-committee-process.md v3.23 §17 + §13.4
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.48-region-allocation-design.md`

## 1. Test Scope

Stage 15.48 is a **design-only stage** — no code changes.

| Area | Test type | Count |
|------|-----------|-------|
| Design doc existence + coverage | Manual review | 1 (checklist) |
| Regression (existing tests) | All | 226 lib + 2085 integration + 5216 conformance |

## 2. Design Doc Coverage Checklist

- [x] Problem statement (region inference is a no-op).
- [x] Current state (1472 LOC infrastructure, `Region::Erased` everywhere).
- [x] Design (lifetime elision, MIR region assignment, constraint collection, error reporting).
- [x] What's already implemented (Stages 7.1-7.5).
- [x] What needs to be implemented (5 items).
- [x] Migration strategy (5 stages, 15.48-15.52).
- [x] Dependencies (Task 7 — COMPLETE, infrastructure — EXISTS).
- [x] Testing strategy (unit + integration + conformance).
- [x] API naming compliance (§23).
- [x] Open questions (4 items).

## 3. Expected Results

- **Lib tests**: 226/226 PASS (zero regression)
- **Conformance tests**: 5216/5216 PASS (zero regression)
- **Clippy**: 0 warnings
- **Fmt**: clean
