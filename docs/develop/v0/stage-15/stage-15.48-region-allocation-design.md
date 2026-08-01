# Stage 15.48 — Region Allocation Design Doc (Task 9 Design Alignment)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.173.0 → v0.174.0
> **Process**: stage-committee-process.md v3.23 §13.4 (设计对齐) + §29
> **v0.2 Phase 2 Task 9 (step 1 of 5)**: Proper region allocation (HP-5)
> **Design doc**: `docs/lang-design/26-region-allocation.md`

## 1. Executive Summary

Stage 15.48 is a **design-only stage** — no code changes. It creates the
design document for region allocation (Task 9, HP-5), which is the next
v0.2 Phase 2 task after Drop elaboration (Task 8) was closed as partially
complete in Stage 15.47.

Per §13.4 (设计对齐 — design before implementation): the design doc must
exist before any implementation work begins.

## 2. What Was Done

### 2.1 Created `docs/lang-design/26-region-allocation.md`

The design document covers:
1. **Problem statement**: 1472 LOC of region inference infrastructure exists but is a no-op because all MIR regions are `Region::Erased`.
2. **Design**: Lifetime elision rules, MIR region assignment, constraint collection, error reporting, integration with NLL.
3. **What's already implemented**: `RegionInferenceContext` with `new()`, `region_to_vid()`, `collect_implied_bounds()`, `infer_regions()` (Stages 7.1-7.5).
4. **What needs to be implemented**: Lifetime elision, MIR region assignment, constraint collection from MIR, error reporting, integration.
5. **Migration strategy**: 5 stages (15.48 design + 15.49-15.51 implementation + 15.52 testing/review).
6. **Dependencies**: Task 7 (NLL) — COMPLETE; region inference infrastructure — EXISTS.
7. **API naming compliance (§23)**: `elide_lifetimes`, `assign_mir_regions`, `collect_mir_constraints`.
8. **Open questions**: interaction with `Region::Erased`, function signature lifetimes, interaction with `elaborate_drops`, performance.

### 2.2 Reviewed existing region inference infrastructure

- `src/borrowck/region_inference.rs` — 1472 LOC, built in Stages 7.1-7.5.
- `RegionInferenceContext` with constraint collection, fixpoint iteration, SCC compression, universe tracking.
- `run_region_inference()` called in `check_mir_body_with_dataflow` — currently a no-op.
- All MIR regions are `Region::Erased` or `Region::Static` — no real lifetime annotations.

## 3. Implementation Plan (Stages 15.48-15.52)

| Stage | Description | Effort | Status |
|-------|-------------|--------|--------|
| **15.48** | **Design doc** (this stage) | 0 (doc only) | ✅ DONE |
| 15.49 | Implement lifetime elision rules + MIR region assignment | 2 days | ⏳ NEXT |
| 15.50 | Implement constraint collection from MIR | 2 days | ⏳ PLANNED |
| 15.51 | Implement error reporting + integration | 1 day | ⏳ PLANNED |
| 15.52 | Conformance tests + gate review | 1 day | ⏳ PLANNED |

## 4. Verification

- No code changes — design-only stage.
- All existing tests pass (zero regression).

## 5. Stage Gate Review (self-review per §9.3)

| Check | Status |
|-------|--------|
| Design doc exists (`docs/lang-design/26-region-allocation.md`) | ✅ |
| Design covers problem, solution, migration, testing | ✅ |
| API naming compliance (§23) planned | ✅ |
| Dependencies verified (Task 7 + region inference infrastructure) | ✅ |
| Open questions documented | ✅ |
| No code changes (design-only stage) | ✅ |
| All existing tests pass (zero regression) | ✅ |
