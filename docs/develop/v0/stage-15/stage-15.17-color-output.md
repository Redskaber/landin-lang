# Stage 15.17 — Color Output for Diagnostics

> **Author**: redskaber  
> **Date**: 2026-07-31  
> **Version**: v0.142.0 → v0.143.0

## Summary

Added ANSI color support to the diagnostics system:
- `ColorConfig` enum (Always/Never/Auto)
- `Color` enum (Red/Yellow/Cyan/Green/Bold/Reset)
- `colorize()` helper function
- `format_snippet_colored()` — colored `^^^` underline
- `DiagnosticBuffer::format_with_source_colored()` — colored full display
- 9 new unit tests

## Color mapping

| Level | Color |
|-------|-------|
| Error/Fatal/Bug | Red |
| Warning | Yellow |
| Note | Cyan |
| Help | Green |

## Test Results

| Suite | Before | After | Delta |
|-------|--------|-------|-------|
| Lib | 161 | 170 | +9 |
| Integration | 2006 | 2006 | 0 |
| Conformance | 5216 | 5216 | 0 |
| **Total** | **7383** | **7392** | **+9** |
