# Stage 15.15 — CLI Diagnostics Migration

> **Author**: redskaber
> **Date**: 2026-07-31
> **Version**: v0.140.0 → v0.141.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)

## 1. Executive Summary

Stage 15.15 migrates the CLI (`src/bin/main.rs`) and mini-cargo (`src/cargo.rs`)
from the deprecated `format_for_user` to the new `format_via_diagnostics`
method. This completes the diagnostics integration — all user-facing error
display now goes through the `src/diagnostics/` module (single source of truth).

The old `format_for_user` is marked `#[deprecated]` with a note pointing to
`format_via_diagnostics`. It's kept for backward compatibility with existing
tests.

## 2. Why This Change?

Per user requirement "留意错误系统、显示友好(src/diagnostics/)":
- Stage 15.13 made `src/diagnostics/` the single source of truth for error
  display formatting.
- Stage 15.14 added the bridge (`to_diagnostics` + `format_via_diagnostics`).
- But the CLI still used the old `format_for_user` — not going through the
  diagnostics module.
- This stage migrates the CLI to use `format_via_diagnostics`, completing the
  diagnostics integration.

Per §1.0 原则 3 "显式 > 隐式": the deprecation is explicit.
Per §23 (API Naming): `#[deprecated(note = "...")]` points to the replacement.

## 3. Changes Made

### 3.1 CLI migration (`src/bin/main.rs`)

**Before**:
```rust
let error_str = result
    .errors
    .format_for_user(Some(&source_file.src), Some(&result.interner));
eprintln!("{}", error_str);
eprintln!("error: aborting due to {} error(s)", result.errors.total_count());
```

**After**:
```rust
let source_map = landin_compiler::session::SourceMap::new(&source_file.src);
let error_str = result.errors.format_via_diagnostics(
    &source_file.src,
    &source_file.name,
    &source_map,
    Some(&result.interner),
);
eprintln!("{}", error_str);
```

The new display includes:
- `error[Code]: message` (rustc-style with category code)
- `  --> source_name:line:col`
- Source snippet with `^^^` underline
- `error: aborting due to N previous error(s)` (from `DiagnosticBuffer`)

### 3.2 Mini-cargo migration (`src/cargo.rs`)

Same migration — `format_for_user` → `format_via_diagnostics`.

### 3.3 Deprecated `format_for_user` (`src/driver.rs`)

Added `#[deprecated(since = "0.140.0", note = "Use format_via_diagnostics instead")]`.
The method is kept for backward compatibility with existing tests.

## 4. §29 Stage-End Deep Review

### 4.1 Data flow coverage (§29.1.1)

All user-facing error display now flows through:
```
CompileErrors → to_diagnostics → DiagnosticBuffer → format_with_source → String
```

The `src/diagnostics/` module is the single source of truth.

### 4.2 Architecture review (§29.1.2)

**Structural clarity** ✅ — All error display goes through one path.
**Efficiency** ✅ — No change (same conversion cost).
**Extensibility** ✅ — Future display improvements (color, error codes) only
need to change `src/diagnostics/`.

### 4.3 Hidden problems (§29.1.4)

| Hidden problem | Complexity growth | Stage 15.15 status |
|----------------|-------------------|-------------------|
| Tests still use `format_for_user` | 1× (backward compat) | Acceptable — deprecated but kept |
| `format_for_user` still exists | 1× (future removal) | Deferred — Stage 15.16 |

## 5. Test Results

| Test suite | Before (v0.140.0) | After (v0.141.0) | Delta |
|------------|-------------------|------------------|-------|
| Rust unit (lib) | 153 | 153 | 0 |
| Rust integration | 2006 | 2006 | 0 |
| Conformance | 5216 | 5216 | 0 |
| **Total** | **7375** | **7375** | **0** |

All tests pass. Zero regressions. Zero clippy warnings. fmt clean.
