# Stage 15.18 — CLI Color Output with TTY Auto-Detection

> **Author**: redskaber  
> **Date**: 2026-07-31  
> **Version**: v0.143.0 → v0.144.0

## Summary

Wired `format_with_source_colored` into the CLI (`src/bin/main.rs`) and
mini-cargo (`src/cargo.rs`) with TTY auto-detection. Colors are enabled
when stderr is a terminal, disabled when piped/redirected.

## Changes

- Added `CompileErrors::format_via_diagnostics_colored()` — delegates to
  `DiagnosticBuffer::format_with_source_colored()`
- Updated `src/bin/main.rs` — uses `format_via_diagnostics_colored` with
  `std::io::IsTerminal` TTY auto-detection
- Updated `src/cargo.rs` — same migration

## Behavior

- **Terminal**: colored output (red errors, yellow warnings, etc.)
- **Piped/redirected**: plain text (no ANSI codes)
- Uses `std::io::stderr().is_terminal()` (stable since Rust 1.70)

## Test Results

All 7392 tests pass (170 lib + 2006 integration + 5216 conformance). Zero regressions.
