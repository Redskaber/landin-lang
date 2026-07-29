# Stage 14.73 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.88.0 → v0.89.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.73 fixes GAP-6 (two-phase borrows) — `&mut self` methods can now
call other `&mut self` methods.

## 2. Bug Fixed

### Bug: &mut self calling &mut self method

**Discovery**: Known limitation from Stage 14.68 audit (Counter::inc_by
calling self.inc()).

**Root cause**: When receiver is already a `Ref` (e.g., `self` in `&mut self`
method), codegen created `&self` producing `&&mut T` instead of `&mut T`.

**Fix**: Check if receiver is already a `Ref`. If so, pass it directly.

## 3. Verification

- All 1951 rust tests pass
- All 5156 conformance tests pass (was 5155, +1 new run_ok)
- 0 clippy warnings, fmt clean
- Debug tool: 130/130 pass (100%)
