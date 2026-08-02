# Stage 15.52 — Region Allocation Gate Review (Task 9 Closure)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.177.0 → v0.178.0
> **Process**: stage-committee-process.md v3.23 §9.3 (Stage Gate Review) + §29
> **v0.2 Phase 2 Task 9 (step 5 of 5)**: Proper region allocation (HP-5) — FINAL REVIEW
> **Design doc**: `docs/lang-design/26-region-allocation.md`

## 1. Executive Summary

Stage 15.52 is the **gate review** stage for Task 9 (Region allocation,
HP-5). It adds integration tests verifying the region allocation pipeline
produces no false positives, and formally closes Task 9 as **PARTIALLY
COMPLETE** — the infrastructure is fully integrated but uses simplified
constraints.

**Key findings**:
- The region allocation pipeline is **complete and integrated**:
  1. MIR region assignment (Stage 15.49) — each `&T` gets a fresh `Region::Var(vid)`.
  2. Constraint collection (Stage 15.50) — outlives constraints from MIR.
  3. Region inference (Stage 7.2) — fixpoint iteration with SCC compression.
  4. Error reporting (Stage 15.51) — errors converted to `BorrowError`.
- 6 integration tests verify no false positives on programs with references.
- All 226 lib + 5216 conformance tests pass (zero regression).
- The constraints are **simplified** (call arguments use `region: 'static`
  instead of proper parameter-region constraints). Full constraint
  precision requires `fn_sigs` integration (deferred).

**Decision**: Task 9 is **PARTIALLY COMPLETE**. The infrastructure is
fully integrated and produces no false positives. Full constraint
precision is deferred to a future stage.

## 2. Task 9 Implementation Review (Stages 15.48-15.51)

### 2.1 Stage 15.48 — Design doc ✅
### 2.2 Stage 15.49 — Lifetime elision + MIR region assignment ✅
### 2.3 Stage 15.50 — Constraint collection from MIR ✅
### 2.4 Stage 15.51 — Error reporting + integration ✅

## 3. Testing

### 3.1 New integration tests (6)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_52_ref_program_no_false_positives` | Simple `let r = &x; *r` compiles cleanly |
| 2 | `stage15_52_multiple_refs_no_false_positives` | Multiple references compile cleanly |
| 3 | `stage15_52_fn_with_ref_params_no_false_positives` | Function taking `&i32` params compiles cleanly |
| 4 | `stage15_52_fn_returning_ref_no_false_positives` | Function returning `&i32` compiles cleanly |
| 5 | `stage15_52_loop_with_refs_no_false_positives` | Loop with references compiles cleanly |
| 6 | `stage15_52_struct_with_ref_no_false_positives` | Struct with `&i32` field compiles cleanly |

## 4. Committee Vote: GO-WITH-CONDITIONS

**Decision**: Task 9 (HP-5) is PARTIALLY COMPLETE. Infrastructure fully
integrated, simplified constraints. Full constraint precision deferred.

## 5. Migration Plan (Stages 15.48-15.52) — FINAL

| Stage | Status | Description |
|-------|--------|-------------|
| 15.48 | ✅ DONE (v0.174.0) | Design doc |
| 15.49 | ✅ DONE (v0.175.0) | Lifetime elision + MIR region assignment |
| 15.50 | ✅ DONE (v0.176.0) | Constraint collection from MIR |
| 15.51 | ✅ DONE (v0.177.0) | Error reporting + integration |
| **15.52** | **✅ DONE (v0.178.0)** | **Integration tests + gate review (this stage)** |

**Task 9 (HP-5): PARTIALLY COMPLETE** — infrastructure fully integrated,
simplified constraints, no false positives.
