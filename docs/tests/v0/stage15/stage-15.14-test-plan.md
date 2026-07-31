# Stage 15.14 — Test Plan

> **Date**: 2026-07-31
> **Version**: v0.139.0 → v0.140.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3

## 1. Test Scope

Stage 15.14 bridges `CompileErrors` to the `diagnostics` module via
`to_diagnostics` and `format_via_diagnostics`.

| Area | Test type | Count |
|------|-----------|-------|
| Driver diagnostics integration | Integration | 8 new |
| Regression (existing tests) | All existing | 1998 + 5216 |

## 2. Integration Test Module

**Path**: `tests/v0/stage15/plan/driver_diagnostics_integration_tests.rs`
**Registered as**: `stage15_driver_diagnostics_integration_tests`

### 2.1 Test cases

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_14_lex_errors_to_diagnostics` | lex → Diagnostic with code "Lex" |
| 2 | `stage15_14_parse_errors_to_diagnostics` | parse → Diagnostic with code "Parse" |
| 3 | `stage15_14_resolve_errors_to_diagnostics` | resolve → Diagnostic with code "Resolve" |
| 4 | `stage15_14_trait_errors_to_diagnostics` | trait → Diagnostic with interner resolution |
| 5 | `stage15_14_format_via_diagnostics_rustc_style` | format produces `error[Code]:` + `-->` |
| 6 | `stage15_14_format_via_diagnostics_includes_snippets` | format includes ` \| ` gutter |
| 7 | `stage15_14_empty_errors_empty_diagnostics` | no errors → empty diagnostics |
| 8 | `stage15_14_to_diagnostics_preserves_count` | diagnostic count == total_count |

## 3. Regression Test Strategy

### 3.1 Existing format_for_user unchanged

The existing `format_for_user` method is kept for backward compatibility.
All existing tests that use `format_for_user` must continue to pass.

### 3.2 Conformance tests

All 5216 conformance tests must continue to pass. The new methods are
additive — `compile()` behavior is unchanged.
