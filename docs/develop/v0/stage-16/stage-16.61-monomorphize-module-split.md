# Stage 16.61 — Monomorphize Module Split + Quality Fixes

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.246.0 → v0.247.0
> **Process**: stage-committee-process.md v3.24 §14.4 + §23

## 1. Executive Summary

Stage 16.61 splits the 1751-line `monomorphize.rs` into three sub-modules
per the Deep Review Round 9 recommendation (§4.1 — HIGH priority cohesion
violation). Also fixes `collect_from_ty` visibility and re-exports
`generics_of` from `hir/mod.rs`.

**Changes**:
1. Split `src/mir/monomorphize.rs` (1751 lines) into:
   - `src/mir/monomorphize/mod.rs` — module declarations + re-exports
   - `src/mir/monomorphize/item.rs` — MonoItem + collection (~750 lines)
   - `src/mir/monomorphize/mangle.rs` — naming (~460 lines)
   - `src/mir/monomorphize/layout.rs` — per-mono layouts (~450 lines)
2. Fixed `collect_from_ty` visibility: `pub` → `pub(crate)` (Round 9 §4.6)
3. Re-exported `generics_of` + `build_generics_map` from `hir/mod.rs` (Round 9 §4.7)
4. Removed `collect_from_ty` from public re-exports (now `pub(crate)`)
5. Cleaned up unused imports in test modules

**Test results**: 8081 tests passing (343 lib + 2514 integration + 5224 conformance), 0 failures, 0 warnings.

## 2. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 343/343 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2514/2514 PASS
- **Total: 8081 tests passing, 0 failures, 0 warnings.**
