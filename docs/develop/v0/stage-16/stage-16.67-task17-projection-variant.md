# Stage 16.67 — Task 17 Phase 2: MIR TyKind::Projection

> **Author**: redskaber
> **Date**: 2026-08-05
> **Version**: v0.252.0 → v0.253.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.67 implements Task 17 Phase 2 — adding `TyKind::Projection(DefId, SubstsRef)`
to the MIR type system. This represents unresolved associated type projections
like `<T as Trait>::Item`.

**Changes**:
1. Added `TyKind::Projection(DefId, SubstsRef)` variant in `src/mir/ty.rs`
2. Updated all 8 match sites to handle the new variant:
   - `is_mir_ty_copy_conservative` — non-Copy (conservative)
   - `ty_is_copy` — Copy (conservative, same as Adt)
   - `ty_is_copy_with_resolver` — non-Copy (conservative)
   - `type_kind_to_string` — `"<projection>"`
   - `substitute` — substitute inner substs (like Adt)
   - `collect_from_ty` — collect from inner substs
   - `mangle_ty` — `"Proj_<def_id>_<substs>"`
   - `drop_elaboration` — treat like Adt (needs drop check)
3. Updated `is_mir_ty_copy_conservative` — Projection is non-Copy

**Test results**: 8099 tests passing (353 lib + 2522 integration + 5224 conformance), 0 failures, 0 warnings.

## 2. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 353/353 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2522/2522 PASS
- **Total: 8099 tests passing, 0 failures, 0 warnings.**
