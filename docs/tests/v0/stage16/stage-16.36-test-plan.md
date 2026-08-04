# Stage 16.36 — Test Plan: Emitter Trait Cleanup

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.232.1

## 1. Test Scope

Stage 16.36 removes the dead `emit_output` method from the Emitter trait.
The test plan verifies that no codegen functionality breaks:

1. TextEmitter still produces correct output (via output_with_globals)
2. LLVMSysEmitter still compiles correctly (via to_module)
3. No regressions on any codegen feature

## 2. Test File

- `tests/v0/stage16/plan/stage16_36_emitter_cleanup_tests.rs`
- 10 tests, all passing ✅

## 3. Test Matrix

| # | Test Name | Verifies |
|---|-----------|----------|
| 1 | `stage16_36_basic_codegen` | Basic codegen |
| 2 | `stage16_36_closure_codegen` | Closure codegen |
| 3 | `stage16_36_string_globals` | String globals (emit_string_global) |
| 4 | `stage16_36_vtable_globals` | Vtable globals (emit_vtable_global) |
| 5 | `stage16_36_dyn_trait_const` | Dyn trait const (emit_dyn_trait_const) |
| 6 | `stage16_36_emit_type_helpers` | EmitType helpers |
| 7 | `stage16_36_fat_ptr_type` | Fat pointer type |
| 8 | `stage16_36_nested_closure` | Nested closure |
| 9 | `stage16_36_triple_nested_closure` | Triple-nested closure |
| 10 | `stage16_36_mutable_capture` | Mutable capture |

## 4. References

- Stage 16.36 design: `docs/develop/v0/stage-16/stage-16.36-emitter-trait-cleanup.md`
- Codegen architecture: `docs/graph/codegen/architecture.md`
