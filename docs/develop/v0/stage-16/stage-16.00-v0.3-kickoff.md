# Stage 16.00 — v0.3 Kickoff: Sound Copy Detection Migration Plan

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.224.0 (review only — no code change)
> **Process**: stage-committee-process.md v3.24 §25 (Deep Review) + §29

## 1. Executive Summary

Stage 16.00 is the **v0.3 kickoff review**. It documents the systematic
audit of all remaining simplifications, the sound Copy detection
migration plan, and the v0.3 roadmap.

**Audit result**: v0.2 is COMPLETE. All remaining simplifications are
documented and scoped. The only unsound item (`Adt => true` in
`ty_is_copy`) has its sound replacement implemented and ready
(`with_resolver_and_sigs`), but migration requires updating 199 tests.

## 2. Systematic Audit Summary

### 2.1 Pipeline Coverage: COMPLETE ✅

All 51 MIR IR enum variants are covered by codegen (Stage 15.97).

### 2.2 Error System: COMPLETE ✅

50 sites fixed (27 Span::DUMMY + 22 {:?} + 1 DRY) across all error
categories (Stages 15.80-15.96).

### 2.3 Region Inference: COMPLETE ✅

All-pairs matching (Stage 15.98), complete constraint set (borrow + copy
+ call args + call return, Stages 15.71+15.93), lifetime elision rules
1-3 (Stages 15.90-15.91), explicit lifetime dedup (Stage 15.92).

### 2.4 Sound Copy Detection: INFRASTRUCTURE READY 🔧

`with_resolver_and_sigs` constructor implemented (Stage 15.99). Enabling
it causes 199 test failures because tests use structs without `impl Copy`
and expect them to be Copy.

**Architectural mismatch** (documented):
- MIR lowerer uses `is_mir_ty_copy_conservative` (Adt=false → Move)
- Borrow checker uses `is_copy` (with_fn_sigs → ty_is_copy → Adt=true → Copy)
- The mismatch is resolved by Move-of-Copy skip (Stage 15.73)
- With sound Copy: both agree (Adt=false → Move, is_copy=false → record)
- But ALL structs without `impl Copy` would be moved, breaking 199 tests

### 2.5 Remaining TODOs (3 items, all low priority)

| # | Location | Priority | Notes |
|---|----------|----------|-------|
| 1 | `borrowck/mod.rs:246` Span::DUMMY | Low | Region error span — needs constraint cause tracking |
| 2 | `mir/lower/field_resolution.rs:86` | Low | MirLowerCtxt mutability — internal improvement |
| 3 | `mir/lower/mod.rs:1317` | Low | Explicit lifetime name tracking (already implemented in Stage 15.92 via `lower_hir_ty_to_mir_ty_with_lifetimes`, this TODO is in the legacy `lower_hir_ty_to_mir_ty_with_regions`) |

## 3. v0.3 Migration Plan

### 3.1 Sound Copy Detection Migration (Priority 1)

**Goal**: Enable `with_resolver_and_sigs` in the driver.

**Steps**:
1. Add `impl Copy for S {}` (or `#[derive(Copy)]`) to all test structs
   that are used as Copy. (~199 tests, 2-3 days mechanical work)
2. Enable `with_resolver_and_sigs` in `driver.rs` (1-line change).
3. Verify all 7612 tests pass.
4. Remove `ty_is_copy` unsound function (or mark `#[deprecated]`).

**Risk**: Some tests may have intentional use-after-move semantics that
are currently masked by the unsound Copy. These tests will correctly
fail and need to be flipped to `compile_error`.

### 3.2 Task 3: TraitResolver Keys (Priority 2)

**Goal**: Redesign `TraitResolver` keys from `(trait_name_spur, type_name_spur)`
to `(DefId, SubstsRef)`.

**Unblocks**: Tasks 11 (Monomorphization), 14 (Object safety), 17
(Associated types).

**Effort**: 2 weeks.

### 3.3 Task 11: Monomorphization (Priority 3)

**Goal**: Real generic instantiation + code generation.

**Dependency**: Task 3.

**Effort**: 2-3 weeks.

### 3.4 Task 10: Closure Redesign (Priority 4)

**Goal**: Strategy A — synthesized `call` function per closure.

**Effort**: 2-3 weeks.

### 3.5 Other v0.3 Items

- Task 4: EmitValue typed handle (4-6 weeks)
- Cross-module visibility (separate feature)
- Region error span tracking (constraint cause → span)
- `#[derive(Copy, Clone)]` attribute support

## 4. v0.2 Final Statistics

| Metric | Value |
|--------|-------|
| Total stages | 99 (15.1-15.99) |
| Total tests | 7612 (244 lib + 2144 integration + 5224 conformance) |
| Failures | 0 |
| Warnings | 0 |
| Tasks complete | 10/20 (50%) |
| Tasks deferred | 8 (v0.3 scope) |
| Error system sites fixed | 50 (27 Span::DUMMY + 22 {:?} + 1 DRY) |
| New helpers | 5 (type_kind_to_string, region_vid_to_string, hir_expr_kind_to_string, operand_span, format_without_interner) |
| Remaining TODOs | 3 (all low priority) |
| Pipeline coverage | 51/51 enum variants ✅ |

## 5. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7612 tests passing, 0 failures, 0 warnings.**

## 6. Committee Vote: GO — v0.3 kickoff approved

v0.2 is COMPLETE. The v0.3 roadmap is clear:
1. Sound Copy detection migration (2-3 days)
2. Task 3 TraitResolver keys (2 weeks)
3. Task 11 Monomorphization (2-3 weeks)
4. Task 10 Closure redesign (2-3 weeks)
