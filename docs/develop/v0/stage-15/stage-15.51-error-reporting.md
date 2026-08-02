# Stage 15.51 — Error Reporting + Integration (Region Inference Errors → BorrowErrors)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.176.0 → v0.177.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 2 Task 9 (step 4 of 5)**: Proper region allocation (HP-5)
> **Design doc**: `docs/lang-design/26-region-allocation.md`

## 1. Executive Summary

Stage 15.51 implements error reporting for region inference. When
`infer_regions()` finds constraint violations, they are now converted
to `BorrowError`s with `BorrowErrorKind::LifetimeError` and added to
the error list. Previously, the result was silently ignored (`let _result = ...`).

**Key results**:
- Added `LifetimeError` variant to `BorrowErrorKind`.
- `run_region_inference` now converts `RegionInferenceError` to `BorrowError`.
- All 226 lib + 5216 conformance tests pass (zero regression — no false positives).
- The region inference is now fully integrated: region assignment → constraint collection → inference → error reporting.

## 2. What Was Done

### 2.1 Added `LifetimeError` to `BorrowErrorKind`

```rust
/// Stage 15.51 (HP-5 step 4): Lifetime error from region inference.
LifetimeError,
```

Per §23: `BorrowErrorKind::LifetimeError` follows the `<Noun>Error` convention.

### 2.2 Updated `run_region_inference` to report errors

```rust
let result = ctx.infer_regions();
if let Err(region_errors) = result {
    for err in region_errors {
        let (message, span) = match &err {
            RegionInferenceError::RegionEscapesUniversal { .. } => (...),
            RegionInferenceError::TypeTestFailed { .. } => (...),
        };
        self.errors.push(BorrowError::new(&message, span, BorrowErrorKind::LifetimeError));
    }
}
```

Per §1.0 原則 5 "报错 > 静默": errors are reported, not silently ignored.

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
| 15.50 | ✅ DONE (v0.176.0) | Constraint collection from MIR |
| **15.51** | **✅ DONE (v0.177.0)** | **Error reporting + integration (this stage)** |
| 15.52 | ⏳ NEXT | Conformance tests + gate review |
