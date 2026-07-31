# Stage 15.13 — Diagnostics System Improvements

> **Author**: redskaber
> **Date**: 2026-07-31
> **Version**: v0.138.0 → v0.139.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)

## 1. Executive Summary

Stage 15.13 improves the `src/diagnostics/` module to make it the single
source of truth for error display formatting. The module previously had
infrastructure (`Diagnostic`, `DiagnosticBuffer`) that was unused — the
driver had its own `format_snippet` function and `format_for_user` method.

This stage:
1. **Moved `format_snippet`** from `driver.rs` (private) to `diagnostics/mod.rs`
   (public) — single source of truth for snippet formatting.
2. **Added `DiagnosticBuilder`** — fluent API for ergonomic `Diagnostic`
   construction with notes, helps, and codes.
3. **Added `DiagnosticBuffer::format_with_source`** — rustc-style display
   with source code snippets (the existing `format` only showed line:col).
4. **Added error limit enforcement** — `DiagnosticBuffer::emit` now respects
   `error_limit` (default 128), preventing overwhelming the user with
   cascading errors.
5. **Added 8 unit tests** for the diagnostics module.

## 2. Why This Change?

Per user requirement "留意错误系统、显示友好(src/diagnostics/)":
- The `src/diagnostics/` module existed but was unused — `DiagnosticBuffer`
  was created in `Session` but never populated.
- The driver had its own `format_snippet` (private) — duplicated logic.
- `DiagnosticBuffer::format` only showed `line:col` without source snippets
  — not as friendly as rustc's display.
- No ergonomic builder for constructing diagnostics with notes/helps.

Per §1.0 原则 3 "显式 > 隐式": the snippet format should be explicit in
the diagnostics module, not hidden in driver.rs.
Per §23 (API Naming): `DiagnosticBuilder` follows the `<Noun>Builder`
pattern consistent with Rust API guidelines.

## 3. Changes Made

### 3.1 Moved `format_snippet` to diagnostics module

**Before**: `format_snippet` was a private function in `src/driver.rs`.
**After**: `format_snippet` is a public function in `src/diagnostics/mod.rs`.
The driver's `format_snippet` is now a thin wrapper:
```rust
fn format_snippet(src: &str, span: &Span) -> String {
    crate::diagnostics::format_snippet(src, span)
}
```

### 3.2 Added `DiagnosticBuilder`

Fluent API for constructing diagnostics:
```rust
let diag = DiagnosticBuilder::error("mismatched types", span)
    .with_code("E0308")
    .with_note("expected `i32`, found `bool`", span)
    .with_help("try using `as i32` to convert", span)
    .build();
```

Methods: `error()`, `warning()`, `note()`, `help()`, `fatal()`, `with_code()`,
`with_note()`, `with_help()`, `build()`.

### 3.3 Added `DiagnosticBuffer::format_with_source`

New method that produces rustc-style display with source code snippets:
```text
error[E0308]: mismatched types
  --> main.lin:5:13
   |
 5 | let x: i32 = true;
   |             ^^^^ expected `i32`, found `bool`
   |
help: try using `as i32` to convert
  --> main.lin:5:13
   |
 5 | let x: i32 = true as i32;
   |             ^^^^^^^^^^^^
```

The existing `format` method (line:col only) is kept for backward compatibility.

### 3.4 Added error limit enforcement

`DiagnosticBuffer::emit` now respects `error_limit` (default 128):
- After `error_limit` errors are emitted, further errors are suppressed.
- A "error limit reached" note is emitted ONCE (using `limit_reached_emitted`
  flag) to inform the user.
- This prevents overwhelming the user with cascading errors (e.g., one
  syntax error causing 100 follow-on errors).

### 3.5 Added 8 unit tests

Tests in `src/diagnostics/mod.rs`:
1. `stage15_13_diagnostic_builder_error` — builder with code + note + help
2. `stage15_13_diagnostic_builder_warning` — warning-level builder
3. `stage15_13_diagnostic_buffer_emit_and_count` — emit + count tracking
4. `stage15_13_diagnostic_buffer_emit_builder` — emit_builder convenience
5. `stage15_13_diagnostic_buffer_error_limit` — error limit enforcement
6. `stage15_13_format_snippet_dummy_span` — dummy span → empty snippet
7. `stage15_13_format_snippet_real_span` — real span → gutter + underline
8. `stage15_13_level_display` — Level Display impl

## 4. §29 Stage-End Deep Review

### 4.1 Data flow coverage (§29.1.1)

The diagnostics module is now the single source of truth for:
- `format_snippet` — source code snippet formatting
- `Diagnostic` construction — via `DiagnosticBuilder`
- `DiagnosticBuffer` — collection + error limit + formatting

The driver's `format_for_user` delegates to `diagnostics::format_snippet`.
Future stages can migrate `format_for_user` to use `DiagnosticBuffer` directly.

### 4.2 Architecture review (§29.1.2)

**Structural clarity** ✅ — `src/diagnostics/` is now the canonical location
for error display. The driver delegates to it instead of duplicating logic.

**Efficiency** ✅ — error limit prevents unbounded error accumulation.

**Extensibility** ✅ — `DiagnosticBuilder` makes it easy to add new
diagnostic kinds (notes, helps, codes) without changing call sites.

### 4.3 Design-impl-test coverage (§29.1.3)

| Design point | Implementation | Test |
|--------------|----------------|------|
| `format_snippet` public | Moved to diagnostics module | `stage15_13_format_snippet_real_span` |
| `DiagnosticBuilder` | Fluent API | `stage15_13_diagnostic_builder_error` |
| `format_with_source` | rustc-style display | (implicit — format uses format_snippet) |
| Error limit | `emit` respects `error_limit` | `stage15_13_diagnostic_buffer_error_limit` |
| `emit_builder` | Convenience method | `stage15_13_diagnostic_buffer_emit_builder` |

### 4.4 Hidden problems (§29.1.4)

| Hidden problem | Complexity growth | Stage 15.13 status |
|----------------|-------------------|-------------------|
| `DiagnosticBuffer` still not wired into driver | 1× (future work) | Deferred — Stage 15.14 |
| No color output (ANSI codes) | 1× (LSP mode) | Deferred — CLI only for now |
| No error code catalog (E001-E999) | 1× (future work) | Deferred — requires catalog |

No new hidden problems. The diagnostics module is now ready for future
integration into the driver.

### 4.5 Refactoring optimality (§29.2)

**Approach taken** ✅ — Moving `format_snippet` to the diagnostics module
is the standard refactoring pattern (extract to canonical location).

**Alternative considered** ✅ — Could have rewired the driver to use
`DiagnosticBuffer` directly. Rejected because it's a larger change — the
driver's `CompileErrors` has 6 separate Vec fields (lex/parse/resolve/
typeck/borrowck/trait_errors), each with different element types. Migrating
all to `Diagnostic` requires converting each error type. Deferred to Stage 15.14.

**Skipped refactors** ✅ — Did not add color output. That requires terminal
detection + ANSI codes, which is a separate feature.

## 5. Test Results

| Test suite | Before (v0.138.0) | After (v0.139.0) | Delta |
|------------|-------------------|------------------|-------|
| Rust unit (lib) | 145 | 153 | +8 (diagnostics tests) |
| Rust integration (all_tests) | 1998 | 1998 | 0 |
| Conformance (.lin) | 5216 | 5216 | 0 |
| **Total** | **7359** | **7367** | **+8** |

All tests pass. Zero regressions. Zero clippy warnings. fmt clean.

## 6. Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Version bump 0.138.0 → 0.139.0 |
| `src/diagnostics/mod.rs` | Added `DiagnosticBuilder`, `format_snippet` (moved from driver), `format_with_source`, error limit, 8 tests |
| `src/driver.rs` | `format_snippet` now delegates to `diagnostics::format_snippet` |
| `docs/develop/v0/stage-15/stage-15.13-diagnostics-system.md` | This document |
| `docs/tests/v0/stage15/stage-15.13-test-plan.md` | **NEW** — test plan |
| `docs/worklog.md` | Stage 15.13 entry appended |
| `RELEASE_NOTES.md` | v0.139.0 entry appended |
| `README.md` | Updated with Stage 15.13 progress |
