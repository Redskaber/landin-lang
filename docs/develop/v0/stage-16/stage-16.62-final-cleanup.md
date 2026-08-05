# Stage 16.62 — Final Project Cleanup: Dead Code Gating + README Rewrite

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.247.0 → v0.248.0
> **Process**: stage-committee-process.md v3.24 §25 (Deep Review) + §1.0 原則 5

## 1. Executive Summary

Stage 16.62 is a final cleanup stage that gates 4 test-only public APIs
behind `#[cfg(test)]` or `#[doc(hidden)]`, removing them from the
production public API surface. This follows the Deep Review Round 9
audit finding that these APIs had zero production callers.

**Changes**:
1. `MonoItem::debug_string()` + `kind_str()` — gated behind `#[cfg(test)]`
2. `MonoLayoutKey::from_mono_item()` — gated behind `#[cfg(test)]`
3. `build_generics_map()` — gated behind `#[cfg(test)]`
4. `substitute_substs()` — marked `#[doc(hidden)]` (needed by integration tests)
5. Removed unused imports: `HashMap` from `generics.rs`, `SubstsRef` from `substitute.rs`
6. Updated `mir/mod.rs` and `hir/mod.rs` re-exports accordingly

**Audit results**:
- 0 clippy warnings (production build)
- 0 `#[allow(unused)]` annotations
- 0 TODO/FIXME in `src/`
- 2 documented `#[allow(dead_code)]` (intentional: `async_marker`, `region_inference`)
- Source: 53,563 lines | Tests: 46,394 lines | Total: 99,957 lines

**Test results**: 8081 tests passing (343 lib + 2514 integration + 5224 conformance), 0 failures, 0 warnings.

## 2. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 343/343 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2514/2514 PASS
- **Total: 8081 tests passing, 0 failures, 0 warnings.**
