# Stage 14.74 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.89.0 → v0.90.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.74 adds `&mut T → &T` coercion (immutable reborrow), completing the
GAP-6 fix. `&mut self` methods can now call both `&self` and `&mut self` methods.

## 2. Bug Fixed

### Bug: &mut self calling &self method failed

**Root cause**: `unify` rejected `Ref(Mut, T)` vs `Ref(Immut, T)`. In Rust,
`&mut T` is a subtype of `&T`.

**Fix**: Allow `Ref(Mut, T)` to unify with `Ref(Immut, T)` (subtype coercion).

## 3. Verification

- All 1951 rust tests pass (1 test updated)
- All 5157 conformance tests pass (was 5156, +1 new run_ok)
- 0 clippy warnings, fmt clean
- Debug tool: 131/131 pass (100%)
