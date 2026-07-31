# Stage 15.14 — Driver Diagnostics Integration

> **Author**: redskaber
> **Date**: 2026-07-31
> **Version**: v0.139.0 → v0.140.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)

## 1. Executive Summary

Stage 15.14 bridges `CompileErrors` (the driver's 6-field error collection)
to the `diagnostics` module (the single source of truth for error display).
This completes the diagnostics integration started in Stage 15.13.

Two new methods on `CompileErrors`:
1. **`to_diagnostics(interner)`** — converts all 6 error types to
   `Diagnostic` values with category codes ("Lex", "Parse", "Resolve",
   "Type", "Borrow", "Trait") and structured notes (expected/found for
   type errors).
2. **`format_via_diagnostics(src, source_name, source_map, interner)`** —
   converts to diagnostics, then formats via
   `DiagnosticBuffer::format_with_source` (rustc-style display).

The existing `format_for_user` is kept for backward compatibility. Future
stages can migrate callers to `format_via_diagnostics`.

## 2. Why This Change?

Per user requirement "留意错误系统、显示友好(src/diagnostics/)":
- Stage 15.13 made `src/diagnostics/` the single source of truth for
  error display formatting (added `format_snippet`, `DiagnosticBuilder`,
  `format_with_source`).
- But the driver's `CompileErrors` still had its own `format_for_user`
  method with inline formatting logic — not using the diagnostics module.
- This stage bridges the gap: `CompileErrors` can now convert to
  `Diagnostic` values and format via the diagnostics module.

Per §1.0 原则 3 "显式 > 隐式": the conversion from typed errors to
`Diagnostic` is explicit in `to_diagnostics`.
Per §23 (API Naming): `to_diagnostics` follows `<verb>_<noun>` pattern;
`format_via_diagnostics` follows `<verb>_<prep>_<noun>` pattern.

## 3. Changes Made

### 3.1 `CompileErrors::to_diagnostics`

```rust
pub fn to_diagnostics(&self, interner: Option<&Rodeo>) -> Vec<Diagnostic>
```

Converts all 6 error types to `Diagnostic` values:
- `lex` → `DiagnosticBuilder::error(msg, span).with_code("Lex").build()`
- `parse` → `DiagnosticBuilder::error(msg, span).with_code("Parse").build()`
- `resolve` → `DiagnosticBuilder::error(msg, span).with_code("Resolve").build()`
- `typeck` → `DiagnosticBuilder::error(msg, span).with_code("Type")`
  + `.with_note("expected: ...", span)` if expected/found present
  + `.with_note("found: ...", span)`
- `borrowck` → `DiagnosticBuilder::error(format!("{} ({:?})", msg, kind), span).with_code("Borrow").build()`
- `trait_errors` → `DiagnosticBuilder::error(format_with_interner(msg), DUMMY).with_code("Trait").build()`

### 3.2 `CompileErrors::format_via_diagnostics`

```rust
pub fn format_via_diagnostics(
    &self,
    src: &str,
    source_name: &str,
    source_map: &SourceMap,
    interner: Option<&Rodeo>,
) -> String
```

Converts to diagnostics, emits to `DiagnosticBuffer`, then formats via
`format_with_source` (rustc-style display with source snippets).

### 3.3 8 new integration tests

Tests in `tests/v0/stage15/plan/driver_diagnostics_integration_tests.rs`:
1. `stage15_14_lex_errors_to_diagnostics` — lex errors convert with code "Lex"
2. `stage15_14_parse_errors_to_diagnostics` — parse errors convert with code "Parse"
3. `stage15_14_resolve_errors_to_diagnostics` — resolve errors convert with code "Resolve"
4. `stage15_14_trait_errors_to_diagnostics` — trait errors convert with interner resolution
5. `stage15_14_format_via_diagnostics_rustc_style` — produces `error[Code]:` + `-->` location
6. `stage15_14_format_via_diagnostics_includes_snippets` — includes ` | ` gutter
7. `stage15_14_empty_errors_empty_diagnostics` — no errors → empty diagnostics
8. `stage15_14_to_diagnostics_preserves_count` — diagnostic count == total_count

## 4. §29 Stage-End Deep Review

### 4.1 Data flow coverage (§29.1.1)

Data flow is now:
```
CompileErrors (6 Vec fields)
  → to_diagnostics(interner)
  → Vec<Diagnostic>
  → DiagnosticBuffer::emit
  → DiagnosticBuffer::format_with_source
  → String (rustc-style display)
```

The diagnostics module is now the single source of truth for formatting.

### 4.2 Architecture review (§29.1.2)

**Structural clarity** ✅ — `CompileErrors` (typed error collection) is
separated from `Diagnostic` (display format). The conversion is explicit.

**Efficiency** ✅ — conversion is O(N) where N = total error count. No
redundant allocation.

**Extensibility** ✅ — adding new error types means adding a new `for e in &self.X` loop in `to_diagnostics`.

### 4.3 Design-impl-test coverage (§29.1.3)

| Design point | Implementation | Test |
|--------------|----------------|------|
| lex → Diagnostic | `to_diagnostics` loop | `stage15_14_lex_errors_to_diagnostics` |
| parse → Diagnostic | `to_diagnostics` loop | `stage15_14_parse_errors_to_diagnostics` |
| resolve → Diagnostic | `to_diagnostics` loop | `stage15_14_resolve_errors_to_diagnostics` |
| trait → Diagnostic (with interner) | `to_diagnostics` loop | `stage15_14_trait_errors_to_diagnostics` |
| format rustc-style | `format_via_diagnostics` | `stage15_14_format_via_diagnostics_rustc_style` |
| format with snippets | `format_via_diagnostics` | `stage15_14_format_via_diagnostics_includes_snippets` |
| empty → empty | `to_diagnostics` | `stage15_14_empty_errors_empty_diagnostics` |
| count preserved | `to_diagnostics` | `stage15_14_to_diagnostics_preserves_count` |

### 4.4 Hidden problems (§29.1.4)

| Hidden problem | Complexity growth | Stage 15.14 status |
|----------------|-------------------|-------------------|
| `format_for_user` still exists (old path) | 1× (backward compat) | Kept — future stage can remove |
| No color output (ANSI codes) | 1× (LSP mode) | Deferred — CLI only for now |
| `format_via_diagnostics` not used by CLI | 1× (future work) | Deferred — Stage 15.15 |

No new hidden problems. The bridge is additive — no existing behavior changed.

### 4.5 Refactoring optimality (§29.2)

**Approach taken** ✅ — Bridging via `to_diagnostics` + `format_via_diagnostics`
is the standard adapter pattern. The existing `format_for_user` is kept for
backward compatibility.

**Alternative considered** ✅ — Could have replaced `format_for_user` entirely.
Rejected because it's a larger change — all callers (main.rs, cargo.rs, tests)
would need updating. Deferred to a future stage.

**Skipped refactors** ✅ — Did not migrate CLI to use `format_via_diagnostics`.
That's a separate change that needs UX testing.

## 5. Test Results

| Test suite | Before (v0.139.0) | After (v0.140.0) | Delta |
|------------|-------------------|------------------|-------|
| Rust unit (lib) | 153 | 153 | 0 |
| Rust integration (all_tests) | 1998 | 2006 | +8 (driver diagnostics tests) |
| Conformance (.lin) | 5216 | 5216 | 0 |
| **Total** | **7367** | **7375** | **+8** |

All tests pass. Zero regressions. Zero clippy warnings. fmt clean.

## 6. Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Version bump 0.139.0 → 0.140.0 |
| `src/driver.rs` | Added `CompileErrors::to_diagnostics` + `format_via_diagnostics` |
| `tests/v0/stage15/plan/driver_diagnostics_integration_tests.rs` | **NEW** — 8 integration tests |
| `tests/all_tests.rs` | Registered `stage15_driver_diagnostics_integration_tests` |
| `docs/develop/v0/stage-15/stage-15.14-driver-diagnostics.md` | This document |
| `docs/tests/v0/stage15/stage-15.14-test-plan.md` | **NEW** — test plan |
| `docs/worklog.md` | Stage 15.14 entry appended |
| `RELEASE_NOTES.md` | v0.140.0 entry appended |
| `README.md` | Updated with Stage 15.14 progress |
