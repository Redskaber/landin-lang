# Stage 14.109 — Gate Review: Performance Optimization (Env Var Caching)

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.122.0 → v0.123.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 deep review)

## 1. Stage Summary

Stage 14.109 implements the first performance optimization from the Phase 2
audit: caching `LANDIN_DEBUG_CODEGEN` env var lookups. This eliminates 8
syscalls per compile, reducing compile-time overhead for debug-mode builds.

## 2. What Was Done

### Env Var Caching (Phase 2 audit recommendation D.3.1)

**Before**: `std::env::var("LANDIN_DEBUG_CODEGEN").is_ok()` was called 8 times
per compile (5 in driver.rs, 3 in codegen/llvm/mod.rs). Each call is a syscall
that traverses the process environment.

**After**: Added `debug_codegen_enabled()` and `debug_borrowck_enabled()` functions
in `src/session/mod.rs` using `OnceLock<bool>` for one-time initialization.
All 8 call sites now use the cached function.

```rust
static DEBUG_CODEGEN: OnceLock<bool> = OnceLock::new();

pub fn debug_codegen_enabled() -> bool {
    *DEBUG_CODEGEN.get_or_init(|| std::env::var("LANDIN_DEBUG_CODEGEN").is_ok())
}
```

### Files Changed

- `src/session/mod.rs`: Added `debug_codegen_enabled()` + `debug_borrowck_enabled()`
- `src/driver.rs`: 5 call sites updated
- `src/codegen/llvm/mod.rs`: 3 call sites updated

### Performance Impact

- Eliminates 7 redundant syscalls per compile (first call caches, rest are O(1))
- Negligible improvement for single-file compiles (<1ms)
- Measurable improvement for LSP-mode long-running compilers (v0.2)
- Per §1.0 原则 6 "通用 > 特例": one cached function serves all 8 call sites

## 3. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5216 passed, 0 failed
```

## 4. Performance Baseline (post-optimization)

- `fib(30)`: compile 9ms (unchanged — env var caching is negligible at this scale)
- 100×100 nested loops: compile+run 57ms (unchanged)

## 5. Stage Verdict

**PASS** — Env var caching implemented. All tests pass. No regressions.
Foundation for v0.2 LSP-mode performance.

v0.123.0: minor bump (performance optimization — env var caching)
