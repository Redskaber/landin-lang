# Stage 15.87 — Resolve Error Span Accuracy Fix (scan_ty_for_unresolved)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.211.0 → v0.212.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.87 fixes a `Span::DUMMY` error site in the resolve error
system. The `scan_ty_for_unresolved` function in `src/driver.rs` used
`Span::DUMMY` for the "cannot find type in this scope" error, producing
"1:1" (file start) instead of the actual type name location.

**Fix**: Use `p.span` (the type path's span) instead of `Span::DUMMY`.

**Before** (`let x: Undefined = 42;`):
```
error[E300]: cannot find type in this scope
  --> /tmp/t.lin:1:1
```

**After**:
```
error[E300]: cannot find type in this scope
  --> /tmp/t.lin:1:20
  |
1 | fn main() { let x: Undefined = 42; }
  |                    ^^^^^^^^^^^
```

**Test impact**:
- 1 new Rust integration test for span accuracy
- 0 conformance test changes
- **Total: 7592 tests passing** (235 lib + 2141 integration [was 2140,
  +1 new] + 5216 conformance), 0 failures, 0 warnings.

Per §1.0 原則 3 "显式 > 隐式": error spans are explicitly sourced
from the type path, not defaulted to Span::DUMMY.
Per §1.0 原則 4 "报错 > 静默": error locations are accurate, not cryptic.

## 2. Why This Matters

The `scan_ty_for_unresolved` function scans HIR types for unresolved
paths (e.g., `let x: Undefined = 42;` where `Undefined` is not a known
type). Previously, the error showed "1:1" (file start) because the
span was hardcoded to `Span::DUMMY`.

This was the last remaining `Span::DUMMY` in the resolve error path:
- `scan_expr_for_unresolved` (value paths) — already uses `p.span` ✅
- `scan_pat_for_unresolved` (pattern paths) — already uses `path.span` ✅
- `scan_ty_for_unresolved` (type paths) — **was `Span::DUMMY`, now `p.span`** ✅

The fix completes the resolve error span accuracy work, matching the
typeck (Stages 15.81-15.83) and borrowck (Stages 15.85) span accuracy
work.

## 3. The Fix

### 3.1 `scan_ty_for_unresolved` span fix (`src/driver.rs`)

```rust
// Before
fn scan_ty_for_unresolved(ty: &crate::hir::HirTy, errors: &mut CompileErrors) {
    match &ty.kind {
        HirTyKind::Path(_, p) => {
            if matches!(p.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::new(
                    "cannot find type in this scope".to_string(),
                    Span::DUMMY,  // ← always "1:1"
                ));
            }
        }
        ...
    }
}

// After
fn scan_ty_for_unresolved(ty: &crate::hir::HirTy, errors: &mut CompileErrors) {
    match &ty.kind {
        HirTyKind::Path(_, p) => {
            if matches!(p.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::new(
                    "cannot find type in this scope".to_string(),
                    p.span,  // ← actual type path span
                ));
            }
        }
        ...
    }
}
```

The `HirPath` struct has a `pub span: Span` field, so `p.span` is
directly available.

## 4. API Naming Compliance (§23)

No API surface changes. The fix modifies 1 error site inside
`scan_ty_for_unresolved` (private free function in `driver.rs`). No
new public functions, types, or re-exports.

## 5. §16 Interface Isolation

The fix is entirely within `driver::scan_ty_for_unresolved`. It reads
`HirPath.span` (HIR data, already available in the function) — no
cross-stage access. The driver is the sole orchestrator that scans HIR
for unresolved paths (per §16.6 exception: driver may read HIR).

## 6. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | Follows the span-from-HIR pattern used in scan_expr/scan_pat |
| D2 Tech Debt | ✅ | 1 more `Span::DUMMY` site fixed (last in resolve error path) |
| D3 Test Coverage | ✅ | 1 new integration test verifies span accuracy |
| D4 Next-Phase Readiness | ✅ | No regressions; resolve errors now have accurate spans |
| D5 Design Rationality | ✅ | Consistent with scan_expr_for_unresolved (uses p.span) |
| D6 Performance | ✅ | No change (same code, different span value) |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | Type resolution error path has span accuracy test |

**Committee Vote**: GO — Stage 15.87 complete.

## 7. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 235/235 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2141/2141 PASS (was 2140, +1 new)
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- **Total: 7592 tests passing, 0 failures, 0 warnings.**

## 8. Error System Cleanup Summary (Stages 15.80-15.87)

The eight-stage error system cleanup is now complete:

| Stage | Focus | Sites Fixed |
|-------|-------|-------------|
| 15.80 | Human-readable type names (`type_to_string`) | 6 `{:?}` leaks + 2 `({:?})` enum leaks |
| 15.81 | Typeck terminator span accuracy (`operand_span`, `term.span`) | 7 `Span::DUMMY` sites + 1 `{:?}` leak |
| 15.82 | Typeck statement/rvalue span accuracy (`stmt_span` in `infer_rvalue`) | 9 `Span::DUMMY` sites + 5 `{:?}` leaks |
| 15.83 | Typeck aggregate (Array + Adt) span accuracy | 2 `Span::DUMMY` sites |
| 15.84 | Borrowck Debug leaks (`region_vid_to_string`) | 3 `{:?}` leaks |
| 15.85 | Borrowck terminator span accuracy (`operand_span`) | 4 `Span::DUMMY` sites |
| 15.86 | DRY refactor: unify `operand_span` into `mir::place` | 1 duplicate eliminated |
| 15.87 | Resolve error span accuracy (`scan_ty_for_unresolved`) | 1 `Span::DUMMY` site |
| **Total** | | **25 `Span::DUMMY` sites + 17 `{:?}` leaks fixed + 1 DRY** |

**Result**: All user-facing typeck, borrowck, AND resolve error messages now:
- Use human-readable type names (`i32`, `bool`, `&mut T`, etc.) — no
  Debug format leaks
- Use human-readable region names (`'r5`, `'r2`) — no `RegionVid(N)`
  Debug leaks
- Point to actual source locations (with snippet underlines) — no
  `Span::DUMMY` / "1:1" errors

The error system is now in good shape for user-facing work.

## 9. Next Steps

The error system cleanup is complete. The next major v0.2 task is:

**Task 12 (Lifetime elision)** — the next major v0.2 task (2-3 weeks,
P1, ready now). This is the last remaining P1 task for v0.2 release.

## 10. Version Policy

v0.211.0 → v0.212.0 (minor bump — Phase 2 resolve error span accuracy
fix + 1 new integration test).
