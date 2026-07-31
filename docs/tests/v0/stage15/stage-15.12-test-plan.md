# Stage 15.12 — Test Plan

> **Date**: 2026-07-31
> **Version**: v0.137.0 → v0.138.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3

## 1. Test Scope

Stage 15.12 makes two error system improvements:
1. Removes `MirBody.lower_type_errors` field (errors returned from lower fn)
2. Improves error display (friendly summary + ResolveError .message)

| Area | Test type | Count |
|------|-----------|-------|
| Error system cleanup | Integration | 8 new |
| Regression (existing tests) | All existing | 1990 + 5216 |

## 2. Integration Test Module

**Path**: `tests/v0/stage15/plan/error_system_cleanup_tests.rs`
**Registered as**: `stage15_error_system_cleanup_tests`

### 2.1 Test cases

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_12_error_summary_friendly_format` | "error: N errors found" format |
| 2 | `stage15_12_singular_error_count` | "1 error found" (singular) |
| 3 | `stage15_12_resolve_error_display` | ResolveError displays via .message (not Debug) |
| 4 | `stage15_12_typeck_error_with_snippet` | typeck errors have snippet gutter |
| 5 | `stage15_12_borrowck_error_with_snippet` | borrowck errors have snippet gutter |
| 6 | `stage15_12_trait_error_display` | trait errors resolve Spur via interner |
| 7 | `stage15_12_no_errors_empty_output` | no errors → empty string |
| 8 | `stage15_12_mirbody_no_lower_type_errors_field` | MirBody field removed (compile-time check) |

## 3. Regression Test Strategy

### 3.1 Updated test files

3 test files updated to use 3-tuple destructuring for the lower fn return.
1 test file updated to assert the new "errors found" format.

### 3.2 Conformance tests

All 5216 conformance tests must continue to pass. The error system changes
are transparent at the user-facing level — `compile()` behavior is unchanged.
