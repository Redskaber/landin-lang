# Stage 15.15 — Test Plan

> **Date**: 2026-07-31
> **Version**: v0.140.0 → v0.141.0

## 1. Test Scope

Stage 15.15 migrates CLI + cargo to `format_via_diagnostics` and deprecates
`format_for_user`. No new tests — existing tests verify no regression.

| Area | Test type | Count |
|------|-----------|-------|
| Regression (existing tests) | All existing | 2006 + 5216 |

## 2. Regression Strategy

### 2.1 Existing tests

All existing tests that use `format_for_user` must continue to pass — the
method is deprecated but kept for backward compatibility.

### 2.2 Conformance tests

All 5216 conformance tests must continue to pass. The CLI migration doesn't
affect `compile()` behavior.
