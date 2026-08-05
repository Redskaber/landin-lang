# Stage 16.65 — Test Plan: Object Safety Driver Integration

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.251.0

## 1. Test Scope

Stage 16.65 wires object safety checking into the driver. Tests verify
that non-object-safe traits used as `dyn Trait` produce compilation errors.

## 2. Test File

- `tests/v0/stage16/plan/stage16_65_object_safety_driver_tests.rs` — 8 tests
- All passing ✅

## 3. Integration Test Coverage (8 tests)

| # | Test | Description |
|---|------|-------------|
| 1 | `safe_trait_dyn_compiles` | Object-safe trait → no error |
| 2 | `self_return_dyn_errors` | Self return → error |
| 3 | `generic_method_dyn_errors` | Generic method → error |
| 4 | `no_receiver_dyn_errors` | No receiver → error |
| 5 | `by_value_self_dyn_errors` | By-value self → error |
| 6 | `self_in_arg_dyn_errors` | Self in arg → error |
| 7 | `ref_mut_self_dyn_compiles` | &mut self → no error |
| 8 | `empty_trait_dyn_compiles` | Empty trait → no error |

## 4. References

- Stage 16.65 design: `docs/develop/v0/stage-16/stage-16.65-task14-driver-integration.md`
