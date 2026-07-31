# Stage 15.13 — Test Plan

> **Date**: 2026-07-31
> **Version**: v0.138.0 → v0.139.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3

## 1. Test Scope

Stage 15.13 improves the `src/diagnostics/` module:
1. Moved `format_snippet` from driver to diagnostics (public)
2. Added `DiagnosticBuilder` for ergonomic construction
3. Added `DiagnosticBuffer::format_with_source` (rustc-style display)
4. Added error limit enforcement

| Area | Test type | Count |
|------|-----------|-------|
| Diagnostics module | Unit (lib) | 8 new |
| Regression (existing tests) | All existing | 1998 + 5216 |

## 2. Unit Test Module

**Path**: `src/diagnostics/mod.rs` (inline `#[cfg(test)] mod tests`)

### 2.1 Test cases

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_13_diagnostic_builder_error` | Builder with code + note + help |
| 2 | `stage15_13_diagnostic_builder_warning` | Warning-level builder |
| 3 | `stage15_13_diagnostic_buffer_emit_and_count` | emit + count tracking |
| 4 | `stage15_13_diagnostic_buffer_emit_builder` | emit_builder convenience |
| 5 | `stage15_13_diagnostic_buffer_error_limit` | Error limit enforcement |
| 6 | `stage15_13_format_snippet_dummy_span` | Dummy span → empty snippet |
| 7 | `stage15_13_format_snippet_real_span` | Real span → gutter + underline |
| 8 | `stage15_13_level_display` | Level Display impl |

## 3. Regression Test Strategy

### 3.1 Driver delegation

The driver's `format_snippet` now delegates to `diagnostics::format_snippet`.
All existing tests that use `format_for_user` (which calls `format_snippet`)
must continue to pass — the output format is identical.

### 3.2 Conformance tests

All 5216 conformance tests must continue to pass. The diagnostics changes
are internal — `compile()` behavior is unchanged.
