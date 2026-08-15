# Stage 18.115 — Span::DUMMY Cleanup (driver.rs) + Enum Branch Audit

> **Author**: redskaber
> **Date**: 2026-08-15
> **Version**: v0.382.0 → v0.383.0
> **Status**: Active

## 1. Audit Summary

Deep audit (Round 3) found:
- **HIGH**: 10 `Span::DUMMY` in `driver.rs` where HIR spans are available
- **MEDIUM**: 5 `Span::DUMMY` in `typeck/checker.rs` where `place_span` is in scope
- **MEDIUM**: 9 `Span::DUMMY` in `typeck/projection_resolver.rs` where `ty.span` is available
- **MEDIUM**: 4 `_ =>` catch-alls that could mask future enum variants

## 2. Fixes

### 2.1 driver.rs Span::DUMMY → HIR span (HIGH)

Propagate `p.span` / `f.sig.span` into `fn_sig_table` construction where HIR
Param/Fn spans are available.

### 2.2 Enum branch coverage documentation (MEDIUM)

Document the 4 `_ =>` catch-alls as intentional grouping with comments noting
which variants are handled and which fall through.

## 3. Span::DUMMY Cleanup Plan

| Category | Count | Action | Target |
|----------|-------|--------|--------|
| (A) Legitimate (synthetic) | ~490 | Leave (macro_expand builtins) | — |
| (B) Should fix (span available) | ~24 | Fix in this stage (driver.rs + checker.rs) | <5 remaining |
| (C) Test code | ~88 | Leave (test infrastructure) | — |
| **Total non-test** | **~602** | **~24 fixed → ~578 remaining** | **95% reduction in fixable** |

## 4. Remaining Span::DUMMY (deferred)

- `typeck/projection_resolver.rs` (9 sites): requires threading `ty.span` through
  recursion — deferred to v0.2 Phase 2 (larger refactor)
- `typeck/where_clause.rs` (5 sites): requires solver context — deferred
- `mir/substitute.rs` (13 sites): documented as intentional (Ty interning)
