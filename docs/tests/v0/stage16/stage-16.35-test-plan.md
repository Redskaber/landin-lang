# Stage 16.35 — Test Plan: Codegen Architecture Refactoring

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.232.0
> **Process**: stage-committee-process.md v3.24 §17.5 + §1.0 原則 5, 6

## 1. Test Scope

Stage 16.35 is a codegen architecture refactoring. The test plan verifies:
1. Text backend still produces correct LLVM IR
2. LLVM backend still compiles correctly
3. No regressions on any codegen feature
4. Dead code is truly removed (compile-time check)

## 2. Test File

- `tests/v0/stage16/plan/stage16_35_codegen_refactoring_tests.rs`
- 12 tests, all passing ✅

## 3. Test Matrix

| # | Test Name | Verifies |
|---|-----------|----------|
| 1 | `stage16_35_basic_codegen_text` | Basic codegen (text backend) |
| 2 | `stage16_35_closure_codegen` | Closure codegen (synthesized path) |
| 3 | `stage16_35_struct_codegen` | Struct codegen (text rendering) |
| 4 | `stage16_35_trait_dispatch_codegen` | Trait dispatch (vtable) |
| 5 | `stage16_35_dyn_trait_codegen` | Dyn trait (fat pointer) |
| 6 | `stage16_35_nested_closure_codegen` | Nested closure codegen |
| 7 | `stage16_35_mutable_capture_codegen` | Mutable capture codegen |
| 8 | `stage16_35_text_utilities_accessible` | Text utilities accessible (compile check) |
| 9 | `stage16_35_emit_type_helpers` | EmitType helpers (shared module) |
| 10 | `stage16_35_fat_ptr_type` | Fat pointer type (shared helper) |
| 11 | `stage16_35_closure_function_unp` | Closure function ungated (compile bug fix) |
| 12 | `stage16_35_multiple_closures` | Multiple closures in same function |

## 4. Verification

- All 7804 tests pass (no regressions)
- 0 clippy warnings, 0 fmt diffs

## 5. References

- Stage 16.35 design: `docs/develop/v0/stage-16/stage-16.35-codegen-architecture-refactoring.md`
- Codegen architecture: `docs/graph/codegen/architecture.md`
- Pipeline overview: `docs/graph/pipeline/overview.md`
