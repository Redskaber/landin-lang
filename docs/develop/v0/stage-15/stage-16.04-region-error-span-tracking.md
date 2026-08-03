# Stage 16.04 — Region Error Span Tracking (Last TODO Resolved)

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.226.2 → v0.226.3
> **Process**: stage-committee-process.md v3.24 §25

## 1. Executive Summary

Stage 16.04 resolves the second-to-last TODO in the codebase: the
`Span::DUMMY` in region error reporting (`borrowck/mod.rs:246`).

**Fix**: Added `span` field to `RegionEscapesUniversal` error variant,
populated from the first matching constraint's cause span. Updated
`borrowck/mod.rs` to use this span instead of `Span::DUMMY`.

**Before**:
```rust
Span::DUMMY, // TODO: track span from constraint cause
```

**After**:
```rust
*span,  // Stage 16.04: use span from constraint cause
```

**Remaining TODOs**: 1 (down from 3 at Stage 16.00)
- `mir/lower/field_resolution.rs:86` — MirLowerCtxt mutability (internal, low priority)

Per §1.0 原則 4 "报错 > 静默": error spans are accurate.
Per §1.0 原則 3 "显式 > 隐式": span is explicitly sourced from constraint cause.

## 2. The Fix

### 2.1 Added `span` field to `RegionEscapesUniversal`

```rust
RegionEscapesUniversal {
    escaping_region: RegionVid,
    universal_region: RegionVid,
    escape_points: Vec<PointIndex>,
    span: crate::session::Span,  // NEW
}
```

### 2.2 Populated span from constraint cause

In `infer_regions()`, when a region escape is detected, the code walks
the constraints list to find the first constraint involving the escaping
region, and extracts its cause span:

```rust
let escape_span = self.constraints.iter()
    .find(|c| c.sup == RegionVid(idx as u32) || c.sub == RegionVid(idx as u32))
    .map(|c| match &c.cause {
        ConstraintCause::FnSignature { span } => *span,
        ConstraintCause::ImpliedBound { span } => *span,
        ConstraintCause::Borrow { span, .. } => *span,
        ConstraintCause::TypeTest { span } => *span,
    })
    .unwrap_or(Span::DUMMY);
```

### 2.3 Updated error reporting

In `borrowck/mod.rs`, the `RegionEscapesUniversal` arm now uses `*span`
instead of `Span::DUMMY`.

## 3. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7612 tests passing, 0 failures, 0 warnings.**

## 4. Version Policy

v0.226.2 → v0.226.3 (patch bump — TODO resolution, span tracking fix).
