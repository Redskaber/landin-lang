# Codegen Enum Tests

> **Author**: redskaber
> **Corresponds to**: `tests/codegen_tests.rs` (Stage 3.38-3.40)
> **Cross-ref**: `docs/develop/v0/stage-3/dev-log.md` Stage 3.38-3.40

## Test Target

Verify enum variant codegen: construction (unit/tuple/struct), discriminant
extraction, match via SwitchInt.

## Covered Scenarios

| Scenario | Test Function | Status |
|----------|--------------|--------|
| Unit variant Red (disc 0) | codegen_enum_unit_variant | PASS |
| Unit variant Green (disc 1) | codegen_enum_unit_variant_second | PASS |
| Unit variant Blue (disc 2) | codegen_enum_unit_variant_third | PASS |
| Tuple variant Some(42) | codegen_enum_tuple_variant | PASS |
| Tuple variant None | codegen_enum_tuple_variant_none | PASS |
| Enum alloca type | codegen_enum_alloca_type | PASS |
| Tuple variant alloca type | codegen_enum_tuple_variant_alloca_type | PASS |
| Store correct type | codegen_enum_variant_store_correct_type | PASS |
| Multiple variants | codegen_multiple_enum_variants | PASS |
| i64 payload | codegen_enum_with_i64_payload | PASS |
| Match with switch | codegen_enum_match_unit_variants | PASS |
| Discriminant extraction | codegen_enum_match_discriminant_extraction | PASS |
| Match with wildcard | codegen_enum_match_with_wildcard | PASS |
| Match returns values | codegen_enum_match_returns_correct_values | PASS |
| Match param type | codegen_enum_match_param_type | PASS |
| Match two variants | codegen_enum_match_two_variants | PASS |
| Match in function | codegen_enum_match_in_function | PASS |
| Non-exhaustive match | codegen_enum_match_non_exhaustive_default | PASS |

**Expected**: 18 | **Actual**: 18 | **Coverage**: 100%
