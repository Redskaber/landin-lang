# Stage 16.51 — Task 11 Phase 1b: Propagate Generic Args into SubstsRef

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.236.1 → v0.237.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.51 implements Task 11 Phase 1b — propagating parsed generic args
from HIR path segments into MIR `SubstsRef`. Previously, all generic args
were silently discarded (always `Vec::new().into()`).

**What was implemented**:
1. `lower_path_generic_args(path, region_counter) -> SubstsRef` — extracts
   `GenericArg::Type` args from `path.segments.last().args`, lowers each to
   MIR `Ty`, collects into `SubstsRef`
2. `lower_ast_ty_to_mir_ty(ty) -> Ty` — minimal AST→MIR type lowerer for
   generic type arguments (handles primitives, paths, tuples)
3. Wired `lower_path_generic_args` into `lower_hir_ty_to_mir_ty_with_regions`
   at the `Res::Def` arm — `Adt(def_id, substs)` now carries actual substs
4. Relaxed `typeck/unify.rs` Adt unification — when substs lengths differ
   (one empty due to incomplete propagation), skip substs comparison and
   match by DefId only (temporary measure until Phase 1c)

**Key result**: `enum Option<T> { Some(T), None } fn main() -> Option<i32>`
now compiles successfully — `Option<i32>` produces `Adt(Option_def_id, [i32])`
in MIR.

**Test results**: 7911 tests passing (250 lib + 2437 integration + 5224
conformance), 0 failures, 0 warnings. All 8 previously-passing generic
conformance tests still pass.

## 2. Changes

### 2.1 New Functions (src/mir/lower/mod.rs)

```rust
/// Lower generic args from HIR path into SubstsRef.
pub(crate) fn lower_path_generic_args(
    path: &HirPath,
    region_counter: &mut u32,
) -> SubstsRef

/// Minimal AST→MIR type lowerer for generic args.
pub(crate) fn lower_ast_ty_to_mir_ty(ty: &ast::Ty) -> mir::ty::Ty
```

### 2.2 Modified: lower_hir_ty_to_mir_ty_with_regions (src/mir/lower/mod.rs)

Before:
```rust
Res::Def(def_id, _) => Ty::new(
    TyKind::Adt(def_id, Vec::<Ty>::new().into()),  // ← empty!
    span,
),
```

After:
```rust
Res::Def(def_id, _) => {
    let substs = lower_path_generic_args(path, region_counter);
    Ty::new(TyKind::Adt(def_id, substs), span)
}
```

### 2.3 Modified: typeck/unify.rs Adt unification

Before: `a_substs.len() != b_substs.len()` → error
After: Only unify substs when both sides have same non-zero length.
If one side has empty substs (not yet propagated from AggregateKind),
skip substs comparison and match by DefId only.

This is a temporary measure until Phase 1c propagates substs into
`AggregateKind::Adt` (struct/enum literal construction).

## 3. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 250/250 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2437/2437 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7911 tests passing, 0 failures, 0 warnings.**

## 4. Version Policy

v0.236.1 → v0.237.0 (minor bump — new functions + typeck behavior change.
Substs are now propagated into Adt types, which changes MIR output.)
