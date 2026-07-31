# Stage 15.6 — Test Plan

> **Date**: 2026-07-31
> **Version**: v0.131.0 → v0.132.0
> **Process**: stage-committee-process.md v3.23 §17 (test standardization)
> + §29.1.3 (design-impl-test coverage)

## 1. Test Scope

Stage 15.6 introduces one behavioral change (cache activation) and one
audit (§23 API naming). The test plan covers both.

| Area | Test type | Count |
|------|-----------|-------|
| `method_return_type_cache` infrastructure | Unit + integration | 6 new |
| §23 API naming audit | Grep-based static check | N/A (no failures) |
| Regression (existing features) | Conformance + Rust integration | 5216 + 1951 (unchanged) |

## 2. New Test Module

**Path**: `tests/v0/stage15/plan/method_return_type_cache_tests.rs`
**Registered as**: `stage15_method_return_type_cache_tests` in `tests/all_tests.rs`

### 2.1 Test cases

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_6_cache_starts_empty` | Fresh `MirLowerCtxt` has empty cache (no spurious entries from construction). |
| 2 | `stage15_6_cache_populates_on_miss_with_no_hir` | When `cx.hir` is `None`, the cache stores `None` for the queried DefId (memoizes negative results). |
| 3 | `stage15_6_repeated_lookups_are_cached` | Repeated lookups of the same DefId produce exactly one cache entry (no duplicates on hit). |
| 4 | `stage15_6_distinct_defids_get_distinct_entries` | Each distinct DefId gets its own cache entry (uniform caching across DefIds). |
| 5 | `stage15_6_cached_matches_uncached_semantics` | The cached `MirLowerCtxt::query_method_return_type` returns the same value as the direct `query_method_return_type_uncached` call. Uses real HIR via `compile()`. |
| 6 | `stage15_6_cache_hit_on_real_hir` | On real HIR, the second lookup of the same DefId does not add a new cache entry (cache hit verified). |

### 2.2 Test design rationale

**Tests 1-4** are infrastructure tests — they verify the cache mechanics
without needing real HIR. They run in microseconds and provide regression
coverage for the cache data structure itself.

**Tests 5-6** are integration tests — they verify the cache works correctly
against real HIR produced by `compile()`. They use a small `struct + impl`
program (`Counter` and `Point`) and find method DefIds by scanning HIR
owners.

**Per §29.1.3** (design-impl-test coverage): the four design points from
`docs/lang-design/19-ty-interning.md` are covered:
- Ty must be Span-free (Test 5 implicitly — cache wouldn't work otherwise)
- Cache stores `Option<Ty>` (Test 2)
- Cached result equals uncached (Test 5)
- Cache hit doesn't re-scan (Tests 3, 6)

### 2.3 Test discovery

Test cases use the `stage15_6_*` naming prefix so they can be filtered
with `cargo test stage15_6` for quick regression checks during development.

## 3. Regression Test Strategy

### 3.1 Rust integration tests (all_tests.rs)

All 1951 existing tests must continue to pass unchanged. The cache is a
transparent memoization layer — no test should observe different behavior.
Run with:

```bash
cargo test --features llvm-backend
```

Expected: 1957 passed (1951 + 6 new), 0 failed, 2 ignored (pre-existing
doc tests).

### 3.2 Conformance tests (.lin files)

All 5216 conformance tests must continue to pass unchanged. The cache
activation does not change *what* the compiler does, only *how fast*.
Run with:

```bash
python3 tests/conformance/run_all.py
```

Expected: 5216 passed, 0 failed.

### 3.3 §23 API naming audit

Static check (no test execution). Verify:

```bash
# No glob re-exports
grep -rEn "^[[:space:]]*pub[[:space:]]+use[[:space:]]+[a-zA-Z_0-9:]+::\*;" src/
# Should output nothing.

# All deprecated items have notes
grep -rn "#\[deprecated" src/ | wc -l   # Count
grep -rn "#\[deprecated(note" src/ | wc -l  # Should match
```

Expected: 0 glob re-exports, 4 deprecated items, all with notes.

## 4. Performance Verification

The cache activation is a performance optimization. To verify it actually
helps, run the benchmark suite before and after:

```bash
# Baseline (v0.131.0)
git stash   # Temporarily revert Stage 15.6
cargo build --release --features llvm-backend
python3 benchmark/run.py > /tmp/before.txt
git stash pop

# After (v0.132.0)
cargo build --release --features llvm-backend
python3 benchmark/run.py > /tmp/after.txt

diff /tmp/before.txt /tmp/after.txt
```

Expected: compile time for crates with heavy method chaining should
improve (5-15% for typical v0.1 crates). Crates with no method calls
should be unchanged (within noise).

## 5. Coverage Matrix

Per §17 test standardization, the coverage matrix for Stage 15.6 is:

| Module | Unit tests | Integration tests | Conformance | Notes |
|--------|-----------|-------------------|-------------|-------|
| `src/mir/lower/mod.rs` (cache method) | lib tests (140 total) | New: 6 cache tests | N/A | Cache method has unit-test coverage via Tests 1-4 |
| `src/mir/lower/expr_operand.rs` (callsites) | Existing 1951 integration | Existing 5216 conformance | All pass | 10 callsites converted; behavior unchanged |
| `src/codegen/mod.rs` (comment cleanup) | N/A | N/A | N/A | Comment-only change, no behavior |
| §23 audit | N/A | N/A | N/A | Static grep check, 0 violations |

## 6. Test File Location

Per §17.3 (test directory standardization):

```
tests/
└── v0/
    └── stage15/
        └── plan/
            └── method_return_type_cache_tests.rs   # NEW
```

The test file follows the `<feature>_tests.rs` naming convention. The
module is registered in `tests/all_tests.rs` under the "Stage 15" section
header, using the `stage15_*_tests` prefix convention.

## 7. Test Maintenance

- **Adding new cache tests**: append to `method_return_type_cache_tests.rs`
  with `stage15_6_*` prefix (or `stage15_7_*` for the next stage).
- **When the cache is replaced by Ty interning (v0.3)**: the cache tests
  should be moved to a `legacy/` subdirectory or deleted, since the
  `method_return_type_cache` field will be removed. Document the deletion
  in the v0.3 stage docs.
- **If a cache test fails after a future change**: investigate whether
  the change introduced a new code path that bypasses the cache. The cache
  is correct by construction — failures indicate a missed callsite.
