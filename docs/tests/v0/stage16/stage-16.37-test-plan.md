# Stage 16.37 — Test Plan: Unified Codegen Pipeline

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.233.0

## 1. Test Scope

Stage 16.37 unifies the codegen pipeline. The test plan verifies:
1. All closure patterns work with unified pipeline
2. Trait dispatch works
3. Drop glue works
4. No regressions

## 2. Test File

- `tests/v0/stage16/plan/stage16_37_unified_pipeline_tests.rs`
- 10 tests, all passing ✅

## 3. Test Matrix

| # | Test Name | Verifies |
|---|-----------|----------|
| 1 | `stage16_37_basic_unified` | Basic codegen |
| 2 | `stage16_37_closure_unified` | Closure codegen |
| 3 | `stage16_37_vtable_unified` | Vtable globals |
| 4 | `stage16_37_dyn_trait_unified` | Dyn trait |
| 5 | `stage16_37_drop_glue_unified` | Drop glue |
| 6 | `stage16_37_nested_closure_unified` | Nested closure |
| 7 | `stage16_37_triple_nested_unified` | Triple-nested closure |
| 8 | `stage16_37_mutable_capture_unified` | Mutable capture |
| 9 | `stage16_37_string_globals_unified` | String globals |
| 10 | `stage16_37_complete_program_unified` | All features together |

## 4. References

- Stage 16.37 design: `docs/develop/v0/stage-16/stage-16.37-unify-codegen-pipeline.md`
- Codegen architecture: `docs/graph/codegen/architecture.md`
