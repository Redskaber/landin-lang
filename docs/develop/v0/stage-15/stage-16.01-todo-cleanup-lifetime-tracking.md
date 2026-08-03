# Stage 16.01 — TODO Cleanup: Lifetime Tracking Documentation Update

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.225.0 → v0.226.0
> **Process**: stage-committee-process.md v3.24 §25 (Deep Review)

## 1. Executive Summary

Stage 16.01 resolves the last TODO in `lower_hir_ty_to_mir_ty_with_regions`
by updating the comment to reflect that lifetime name tracking is now
implemented in `lower_hir_ty_to_mir_ty_with_lifetimes` (Stage 15.92).

**Before**:
```rust
// TODO (future): track the lifetime name so we can unify
// references with the same explicit lifetime.
```

**After**:
```rust
// Stage 15.92: Lifetime name tracking is now implemented in
// `lower_hir_ty_to_mir_ty_with_lifetimes` (uses a
// `lifetime_map: HashMap<Symbol, RegionVid>` for deduplication).
// This legacy function is retained for callers that don't need
// lifetime deduplication (fn_sigs building, self type resolution).
// Use `lower_hir_ty_to_mir_ty_with_lifetimes` for lifetime tracking.
```

## 2. Remaining TODOs (2 items, all low priority)

| # | Location | Priority | Notes |
|---|----------|----------|-------|
| 1 | `borrowck/mod.rs:246` Span::DUMMY | Low | Region error span — needs constraint cause tracking |
| 2 | `mir/lower/field_resolution.rs:86` | Low | MirLowerCtxt mutability — internal improvement |

## 3. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7612 tests passing, 0 failures, 0 warnings.**

## 4. Version Policy

v0.225.0 → v0.226.0 (minor bump — TODO cleanup, documentation update).
