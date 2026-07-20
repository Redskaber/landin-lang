# Codegen Enum Tests

> **Author**: redskaber
> **Corresponds to**: `tests/codegen_tests.rs` (Stage 3.38-3.47)
> **Cross-ref**: `docs/develop/v0/stage-3/dev-log.md` Stage 3.38-3.47

## Test Target

Verify enum variant codegen: construction (unit/tuple/struct), discriminant
extraction, match via SwitchInt, and (Stage 3.47) **AdtLayout::Enum**-driven
storage layout resolution per §16 (no codegen→HIR lookup).

## Stage 3.47 — AdtLayout::Enum Coverage (L-PIPE-1 closure)

Stage 3.47 closed L-PIPE-1's enum extension (silently added in Stage 3.38).
Enum storage layouts are now resolved via `mir.adt_layouts` (populated by
MIR lower from HIR). The `AdtLayout::Enum` variant stores **all** variants'
payloads (forward-compatible with Stage 4's L-ENUM-UNION fix). Codegen
currently uses "first non-empty payload" (preserves Stage 3.38 behavior).

The new enum-related AdtLayout tests (covered by R14 audit `i04`, `i05`,
`i06`, `i13`, `i14`, `e03`, `e07`):

| Scenario | Audit Case | Status |
|----------|-----------|--------|
| Unit-only enum param = `{ i32 }` | i04 | PASS |
| Tuple-variant enum param = `{ i32, i32 }` | i05 | PASS |
| Enum match via AdtLayout discriminant | i06 | PASS |
| Enum struct-variant = `{ i32, i32, i32 }` | i13 | PASS |
| Multi-variant enum (first non-empty) | i14 | PASS |
| All-unit enum (no phantom payload) | e03 | PASS |
| Enum return value uses AdtLayout | e07 | PASS |

**Stage 3.47 expected**: 7 audit cases | **Actual**: 7 | **Coverage**: 100%

## Covered Scenarios (Stage 3.38-3.40)

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

**Stage 3.38-3.40 expected**: 18 | **Actual**: 18 | **Coverage**: 100%

## Forward Compatibility Note (Stage 4 — L-ENUM-UNION)

The `AdtLayout::Enum { discriminant_ty, variant_payloads: Vec<Vec<Ty>> }`
data structure already stores **all** variants' payload types. When Stage 4
closes L-ENUM-UNION (proper union of all variant payloads), the codegen
change will be a single match-arm update in
`mir_type_to_emit_type_with_layouts` — no MIR data-structure change needed.
