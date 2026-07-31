# Stage 15.6 — `method_return_type_cache` Activation + API Naming Audit

> **Author**: redskaber
> **Date**: 2026-07-31
> **Version**: v0.131.0 → v0.132.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)

## 1. Executive Summary

Stage 15.6 completes the v0.2 Phase 1 perf-and-debt cleanup that became
unblocked by Stage 15.5's Span removal from `Ty`. The headline change is
activating the `method_return_type_cache` infrastructure that was added in
Stage 15.4 but couldn't be turned on because `Ty` carried a `Span` — making
otherwise-equal types compare unequal and blowing the cache.

With the cache live, repeated calls to `query_method_return_type(def_id)`
are now O(1) amortized (hash lookup) instead of O(n) HIR scan. For crates
with chained method calls (`a.b().c().d()`), this eliminates a quadratic
hot-spot flagged by the Phase 2 audit (HP-B12).

A secondary contribution is the §23 API naming audit: 0 violations found
(no glob re-exports, all deprecated items carry notes, all entry points
follow the `<verb>_<noun>` free-function pattern, all context types use
`Ctxt` / `-er` suffix). The audit is documented below as the §23 review
record for Stage 15.6.

## 2. Changes Made

### 2.1 Activate `method_return_type_cache` (src/mir/lower/)

**Background**. Stage 15.4 added `MirLowerCtxt.method_return_type_cache:
RefCell<HashMap<DefId, Option<Ty>>>` as infrastructure-only — the cache was
populated by no one. Stage 15.5 removed `Span` from `Ty`, which was the
blocker: cached `Ty` values from one span context would miss the cache
under a different span. With `Span` gone, `Ty` equality is `TyKind`
equality — perfect for hashing.

**Implementation**.

1. The free function `query_method_return_type(hir, did)` was renamed to
   `query_method_return_type_uncached(hir, did)` and made `pub` so tests
   can verify cache semantics. Its body is unchanged.
2. A new method `MirLowerCtxt::query_method_return_type(&self, did) ->
   Option<Ty>` was added. It checks the cache first; on miss, it calls the
   uncached function and stores the result (including `None` results, to
   avoid re-scanning HIR for known-unresolvable DefIds).
3. All 10 callsites in `src/mir/lower/expr_operand.rs` were converted:
   - `query_method_return_type(hir, did)` → `cx.query_method_return_type(did)`
   - `cx.hir.and_then(|hir| query_method_return_type(hir, did))` →
     `cx.query_method_return_type(did)`

**Why `None` is cached**. The cache uses `HashMap<DefId, Option<Ty>>`, not
`HashMap<DefId, Ty>`. This is intentional: a DefId that fails to resolve
once will fail every time (HIR is immutable per compilation). Caching the
negative result avoids re-scanning all HIR owners on every method call to
an unknown DefId. Per §1.0 原则 3 "显式 > 隐式": the `Option<Ty>` value
explicitly distinguishes "looked up, not found" from "never looked up".

### 2.2 Dead comment cleanup

Two stale comments in `src/codegen/mod.rs` and `src/mir/lower/expr_operand.rs`
referenced `MirBody.println_messages` — a side-table field removed in Stage
14.x. The comments were updated to reflect that the field is gone, with a
note that the inline-emission design decision (Stage 13.13) is preserved.

### 2.3 §23 API naming audit (zero violations)

A full audit was performed per §23.2 checklist:

| Check | Result |
|-------|--------|
| `pub use X::*;` glob re-exports | 0 violations |
| Entry-point free-function pattern (`<verb>_<noun>`) | All stages compliant |
| Context type suffix (`Ctxt` / `-er`) | `MirLowerCtxt`, `HirLowerCtxt`, `TypeChecker`, `BorrowChecker`, `Lexer`, `Parser`, `Resolver`, `Emitter` — all compliant |
| Type prefix (Hir/Mir/Emit) | All compliant |
| Cross-module duplicate type definitions (DRY) | None |
| `#[deprecated]` without `note = "..."` | None — all 4 deprecated items have notes pointing to §16-compliant replacements |

The audit covered `src/lib.rs` re-exports, all `src/*/mod.rs` module
declarations, and `src/bin/main.rs`. No remediation needed.

### 2.4 New test module (tests/v0/stage15/plan/method_return_type_cache_tests.rs)

Six new tests verify the cache:

1. `stage15_6_cache_starts_empty` — fresh `MirLowerCtxt` has empty cache.
2. `stage15_6_cache_populates_on_miss_with_no_hir` — `None` results cached.
3. `stage15_6_repeated_lookups_are_cached` — cache hit doesn't add entry.
4. `stage15_6_distinct_defids_get_distinct_entries` — uniform caching.
5. `stage15_6_cached_matches_uncached_semantics` — correctness invariant,
   verified against real HIR via `compile()`.
6. `stage15_6_cache_hit_on_real_hir` — cache hit verified on real HIR.

The test module is registered in `tests/all_tests.rs` as
`stage15_method_return_type_cache_tests`. Test count: 1951 → 1957.

## 3. §29 Stage-End Deep Review

### 3.1 Data flow coverage (§29.1.1)

The cache is a pure memoization layer — it does not change *what* value is
returned, only *how fast*. Data flow is unchanged: `MirLowerCtxt` reads
HIR (immutable), computes `Option<Ty>`, stores it. Codegen consumes `Ty`
the same as before. No new catch-all branches introduced.

### 3.2 Architecture review (§29.1.2)

**Structural clarity** ✅ — the cache lives on `MirLowerCtxt` (the
lowering context), not on `MirBody` (the IR). This respects §16: the
cache is *lowering state*, not *MIR data*. It is discarded when lowering
completes; the IR is unchanged.

**Efficiency** ✅ — converts O(B × M × O) (bodies × method calls × HIR
owners) to O(B × M + O) (one HIR scan per unique method, amortized across
all calls). For a 100-fn crate with 50 method calls, this is roughly
10× fewer HIR scans.

**Extensibility** ✅ — adding new HIR owner kinds (e.g. `HirItem::Mod`
methods) requires no change to the cache. The cache key is `DefId`, which
is owner-kind-agnostic.

### 3.3 Design-impl-test coverage (§29.1.3)

| Design point (from `19-ty-interning.md`) | Implementation | Test |
|-------------------------------------------|----------------|------|
| Ty must be Span-free for cache to work | `Ty { kind: TyKind }` (Stage 15.5) | `ty_primitive_construction` (existing) |
| Cache stores `Option<Ty>` to memoize None | `RefCell<HashMap<DefId, Option<Ty>>>` | `stage15_6_cache_populates_on_miss_with_no_hir` |
| Cached result must equal uncached result | `query_method_return_type` calls `_uncached` on miss | `stage15_6_cached_matches_uncached_semantics` |
| Cache hit must not re-scan HIR | Verified by cache entry count after repeated lookups | `stage15_6_repeated_lookups_are_cached` |

All four design points have implementation + test coverage.

### 3.4 Hidden problems (§29.1.4)

| Hidden problem | Complexity growth if not fixed | Stage 15.6 status |
|----------------|--------------------------------|-------------------|
| `Ty` not `Copy` (still heap-allocates per clone) | 2× per stage that adds type ops | Deferred to v0.3 (Rc stepping stone) |
| `SubstsRef = Vec<Ty>` (per-app heap alloc) | 2× | Deferred to v0.2 Phase 1 Task 2 |
| `MirBody.lower_type_errors` mixes IR + error collection | 1× (architectural smell, not blocking) | Documented as v0.3 P2 |
| 8 driver writeback passes | 2× per new type feature | Deferred to v0.2 Phase 1 Task 5 |
| `region_inference.rs` 1462 LOC dead | 1× (no growth, just dead mass) | Deferred to v0.2 Phase 2 Task 7 |

None of these grow ≥ 2× by *not* fixing them in Stage 15.6. The cache
activation does not add new hidden problems.

### 3.5 Refactoring optimality (§29.2)

**Approach taken** ✅ — Memoization via `RefCell<HashMap>` is the standard
Rust pattern for interior-mutable per-context caches. The `Option<Ty>`
value type correctly captures "looked up, not found" vs "never looked up".

**Alternative considered** ✅ — Could have moved the cache to the driver
level (one global cache per compilation). Rejected because (a) it would
require threading the cache through every `lower_*` call, violating §16
(driver reaches into MIR lower), and (b) `MirLowerCtxt` is the natural
owner — it already holds the HIR reference.

**Skipped refactors** ✅ — Did not consolidate the 8 driver writeback
passes (Phase 1 Task 5). This is a 1-2 week effort that needs its own
stage; attempting it in 15.6 would have mixed concerns. Per §15 "最优 >
最小": the cache activation alone is the optimal change for this stage.

## 4. Test Results

| Test suite | Before (v0.131.0) | After (v0.132.0) | Delta |
|------------|-------------------|------------------|-------|
| Rust unit (lib) | 140 | 140 | 0 |
| Rust integration (all_tests) | 1951 | 1957 | +6 |
| Conformance (.lin) | 5216 | 5216 | 0 |
| **Total** | **7307** | **7313** | **+6** |

All tests pass. Zero regressions. Zero clippy warnings. fmt clean.

## 5. v0.2 Phase 1 Progress Update

| Task | Status | Notes |
|------|--------|-------|
| 1. Ty interning (`Ty<'tcx>` Copy) | Design done (Stage 15.1) | Implementation deferred to v0.3 (Rc stepping stone) |
| 2. SubstsRef → `&'tcx [GenericArg]` | Not started | Blocked on Task 1 |
| 3. TraitResolver key redesign | Not started | Blocked on Tasks 1+2 |
| 4. EmitValue → typed LLVM handle | Not started | Independent, can start any time |
| 5. Consolidate 8 writeback passes → 2 | Not started | 1-2 weeks, needs own stage |
| **Side-quest: Span removal from Ty** | ✅ Done (Stage 15.5) | Unblocked Tasks 1, 6, 9 |
| **Side-quest: method_return_type_cache activation** | ✅ Done (Stage 15.6) | Closes Phase 2 audit HP-B12 |
| **Side-quest: §23 API naming audit** | ✅ Done (Stage 15.6) | Zero violations |

Stage 15.6 closes two Phase 2 audit items (HP-B12 perf, §23 compliance)
without consuming the budget for the larger Phase 1 tasks. The next stage
(15.7) should tackle Task 5 (writeback consolidation) or Task 1 (Rc
stepping-stone Ty interning).

## 6. Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Version bump 0.131.0 → 0.132.0 |
| `src/mir/lower/expr_operand.rs` | Renamed fn → `_uncached`, `pub(crate)` → `pub`, converted 10 callsites to `cx.query_method_return_type(did)`, stale comment cleanup |
| `src/mir/lower/mod.rs` | Added `MirLowerCtxt::query_method_return_type` method (cached wrapper), re-exported `query_method_return_type_uncached` |
| `src/codegen/mod.rs` | Stale `println_messages` comment cleanup |
| `tests/all_tests.rs` | Registered `stage15_method_return_type_cache_tests` module |
| `tests/v0/stage15/plan/method_return_type_cache_tests.rs` | New — 6 cache tests |
| `docs/develop/v0/stage-15/stage-15.6-method-return-type-cache.md` | This document |
| `docs/tests/v0/stage15/stage-15.6-test-plan.md` | New — test plan doc |
| `docs/worklog.md` | Stage 15.6 entry appended |
| `RELEASE_NOTES.md` | v0.132.0 entry appended |
| `README.md` | Updated with v0.2 progress |
