# Stage 15.49 — Lifetime Elision + MIR Region Assignment

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.174.0 → v0.175.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 2 Task 9 (step 2 of 5)**: Proper region allocation (HP-5)
> **Design doc**: `docs/lang-design/26-region-allocation.md`

## 1. Executive Summary

Stage 15.49 implements lifetime elision and MIR region assignment. The
`lower_hir_ty_to_mir_ty` function now delegates to a new
`lower_hir_ty_to_mir_ty_with_regions` function that assigns a fresh
`Region::Var(RegionVid(n))` to each reference type, instead of
`Region::Erased` (which mapped to `'static`).

**Key results**:
- All reference types in MIR now have unique region variables.
- The region inference infrastructure (`RegionInferenceContext`) now has
  real region variables to work with (instead of all mapping to `'static`).
- All 226 lib + 5216 conformance tests pass (zero regression).
- The region inference is still effectively a no-op (it doesn't enforce
  constraints yet — that's Stage 15.50).

## 2. What Was Done

### 2.1 Added `lower_hir_ty_to_mir_ty_with_regions`

New function in `src/mir/lower/mod.rs` that takes a `&mut u32` region
counter and assigns a fresh `Region::Var(RegionVid(n))` to each reference
type (both explicit and elided lifetimes). The counter is incremented per
reference, ensuring each reference gets a unique vid.

The legacy `lower_hir_ty_to_mir_ty` now delegates to this function with
a throwaway counter.

### 2.2 Updated lowering entry point

`lower_hir_body_to_mir_full_with_dyn_trait_plan` now allocates a
`region_counter: u32` and passes `&mut region_counter` to
`lower_hir_ty_to_mir_ty_with_regions` when lowering return types and
parameter types. This ensures all references in a function body get
unique region variables.

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
| **15.49** | **✅ DONE (v0.175.0)** | **Lifetime elision + MIR region assignment (this stage)** |
| 15.50 | ⏳ NEXT | Constraint collection from MIR |
| 15.51 | ⏳ PLANNED | Error reporting + integration |
| 15.52 | ⏳ PLANNED | Conformance tests + gate review |
