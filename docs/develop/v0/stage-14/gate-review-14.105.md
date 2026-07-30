# Stage 14.105 — Gate Review: Dead Code Cleanup + Performance Baseline

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.118.0 → v0.119.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 deep review)

## 1. Stage Summary

Stage 14.105 performs P1 dead code cleanup and establishes a performance
baseline. This is the first stage after all P0 bugs were fixed (Stage 14.104).

## 2. Dead Code Cleanup

### Files Removed (4 files, 1,013 LOC)

| File | LOC | Status | Reason |
|------|-----|--------|--------|
| `src/mir/lvalue.rs` | 250 | Orphaned | Renamed to `place` in Stage 3.66, not declared as module |
| `src/typeck/lifetime_elision.rs` | 215 | `#[allow(dead_code)]` | Never called since Stage 8.1 |
| `src/borrowck/drop_elaboration.rs` | 282 | `#[allow(dead_code)]` | Never called since Stage 8.4 |
| `src/traits/object_safety.rs` | 266 | `#[allow(dead_code)]` | Never called since Stage 8.2 |
| **Total** | **1,013** | | |

### Files Kept

| File | LOC | Reason |
|------|-----|--------|
| `src/borrowck/region_inference.rs` | 1,462 | Partially used — `new`, `region_to_vid`, `collect_implied_bounds`, `infer_regions` are called by `run_region_inference`. The rest (SCC, type tests, universe escape) is dead infrastructure for v0.2. |

### Impact

- Source files: 99 → 95 (-4)
- Source LOC: 42,418 → 41,769 (-649 net, 1,013 deleted - 364 comment replacements)
- No functional change — all tests pass identically
- Removes 4 `#[allow(dead_code)]` attributes, improving code hygiene

## 3. Performance Baseline

### Test 1: Fibonacci (recursive, fib(30))

```
Program: fib(30) = 832040
Compile + run: <1 second total
Compile only: 9ms
```

### Test 2: Nested loops with struct methods (100×100)

```
Program: 100×100 nested loop with Point::new + distance method calls
Result: 20000
Compile + run: 57ms total
```

### Observations

- Compile times are fast (<60ms for moderate programs)
- Runtime performance is good (LLVM -O3 optimization)
- No obvious performance regressions from dead code removal
- Performance hotspots identified in Phase 1 audit (LLVMSysEmitter::lookup,
  cstr() leaks, LANDIN_DEBUG_CODEGEN env var) are noted for future optimization

## 4. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5216 passed, 0 failed
```

## 5. Stage Verdict

**PASS** — 1,013 LOC dead code removed. Performance baseline established.
No regressions. All tests pass identically.

Per §1.0 原则 5 "报错 > 静默": removing `#[allow(dead_code)]` modules
eliminates "activated but no-op" states that could confuse future developers.

Per §14.4 (architectural split): removing orphaned files improves code
organization clarity.

v0.119.0: minor bump (P1 dead code cleanup + performance baseline)
