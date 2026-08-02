# Stage 15.50 — Constraint Collection from MIR

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.175.0 → v0.176.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 2 Task 9 (step 3 of 5)**: Proper region allocation (HP-5)
> **Design doc**: `docs/lang-design/26-region-allocation.md`

## 1. Executive Summary

Stage 15.50 implements `collect_mir_constraints` — a method on
`RegionInferenceContext` that walks MIR statements and terminators,
collecting outlives constraints between regions. This is wired into
`run_region_inference` in `borrowck/mod.rs`, so the region inference
now has real constraints to work with (instead of just implied bounds
from local declarations).

**Key results**:
- `collect_mir_constraints` walks all basic blocks, collecting constraints from:
  - `r = &x` (Rvalue::Ref): borrowed place's regions outlive borrow region.
  - `r = Copy(x)` where x is `&T`: propagate lifetime (src region outlives lhs region).
  - `call f(&x)`: argument regions outlive `'static` (simplified).
- Added `place_ty` helper for looking up a place's type in MIR.
- All 226 lib + 5216 conformance tests pass (zero regression).

## 2. What Was Done

### 2.1 Added `collect_mir_constraints` to `RegionInferenceContext`

New method in `src/borrowck/region_inference.rs`:

```rust
pub(crate) fn collect_mir_constraints(&mut self, mir: &MirBody)
```

Walks all basic blocks. For each `StatementKind::Assign`:
- `Rvalue::Ref(region, _, place)`: extracts regions from the borrowed place's type and adds `r: region` constraints.
- `Rvalue::Use(Operand::Copy/Move(lv))`: if the source type has regions, propagates them to the LHS.

For `TerminatorKind::Call`:
- For each `&T` argument, adds `region: 'static` (simplified — proper parameter constraints in Stage 15.51).

### 2.2 Added `place_ty` helper

A helper method on `RegionInferenceContext` that looks up a place's type
in the MIR body — handles `Local`, `Static`, and `Projection` (Deref,
Field, Index, ConstantIndex, Subslice).

### 2.3 Wired into `run_region_inference`

`src/borrowck/mod.rs::run_region_inference` now calls
`ctx.collect_mir_constraints(mir)` after collecting implied bounds from
local declarations and before running `infer_regions()`.

## 3. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 226/226 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS

## 4. Migration Plan (Stages 15.48-15.52) — Updated

| Stage | Status | Description |
|-------|--------|-------------|
| 15.48 | ✅ DONE (v0.174.0) | Design doc |
| 15.49 | ✅ DONE (v0.175.0) | Lifetime elision + MIR region assignment |
| **15.50** | **✅ DONE (v0.176.0)** | **Constraint collection from MIR (this stage)** |
| 15.51 | ⏳ NEXT | Error reporting + integration |
| 15.52 | ⏳ PLANNED | Conformance tests + gate review |
