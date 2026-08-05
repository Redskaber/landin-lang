# Stage 16.65 — Task 14 Phase 2: Object Safety Driver Integration

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.250.0 → v0.251.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.65 wires the object safety checker (Stage 16.64) into the driver
pipeline. When `dyn Trait` is used with a non-object-safe trait, the compiler
now emits a typeck error.

**Changes**:
1. Added `check_object_safety_for_dyn_trait_usage` function in `src/driver.rs`
   - Scans all HIR bodies for `HirTyKind::TraitObject` usage
   - Also scans fn signatures, struct fields, enum variants
   - For each `dyn Trait`, resolves trait DefId, looks up HirTrait, calls
     `check_trait_object_safety`
   - Emits typeck errors for any violations found

2. Added helper functions:
   - `check_trait_object_ty` — check a single TraitObject type
   - `walk_hir_ty` — recursive type walker
   - `walk_hir_ty_in_body` — walk HirExpr for type annotations
   - `walk_hir_ty_in_stmt` — walk HirStmt for type annotations
   - `walk_hir_block` — walk HirBlock

3. **8 integration tests** in
   `tests/v0/stage16/plan/stage16_65_object_safety_driver_tests.rs`:
   - Safe trait with dyn Trait compiles (2 tests)
   - Self return with dyn Trait errors (1 test)
   - Generic method with dyn Trait errors (1 test)
   - No receiver with dyn Trait errors (1 test)
   - By-value self with dyn Trait errors (1 test)
   - Self in arg with dyn Trait errors (1 test)
   - Empty trait with dyn Trait compiles (1 test)

**Test results**: 8099 tests passing (353 lib + 2522 integration + 5224
conformance), 0 failures, 0 warnings. +8 new integration tests.

## 2. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 353/353 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2522/2522 PASS (+8 new)
- **Total: 8099 tests passing, 0 failures, 0 warnings.**
