# Codegen Enum Tests

> **Author**: redskaber
> **Corresponds to**: `tests/v0/stage3/plan/codegen_tests.rs` (Stage 3.38-3.48)
> **Cross-ref**: `docs/develop/v0/stage-3/dev-log.md` Stage 3.38-3.48

## Test Target

Verify enum variant codegen: construction (unit/tuple/struct), discriminant
extraction, match via SwitchInt, (Stage 3.47) **AdtLayout::Enum**-driven
storage layout resolution per §16 (no codegen→HIR lookup), and (Stage 3.48)
**flat enum storage layout** + **pattern binding extraction** (L-ENUM-UNION +
L-ENUM-BINDING closure).

## Stage 3.48 — L-ENUM-UNION + L-ENUM-BINDING Closure

Stage 3.48 closes two soundness bugs:

1. **L-ENUM-UNION**: enum storage layout was `{ discr, first_non_empty_payload }`
   (Stage 3.38 behavior). For `enum E { A, B(i32), C(i64) }`, this was
   `{ i32, i32 }` — constructing `E::C(42)` would write an 8-byte i64 into
   the 4-byte i32 slot, silently corrupting adjacent memory. **Fix**: flatten
   ALL non-empty variants' payload fields into storage. Case C layout is now
   `{ i32, i32, i64 }`.

2. **L-ENUM-BINDING** (hidden P0): `Opt::Some(x) => x` allocated a local for
   `x` but never assigned it — reading uninitialized memory. Pre-existing
   since Stage 3.40. **Fix**: new `lower_enum_variant_pattern_bindings`
   function generates `binding = Copy(scrut.Field(field_idx, field_ty))`
   projections.

### Stage 3.48 new tests (12)

| Scenario | Test Function | Status |
|----------|--------------|--------|
| Case C layout (≥2 non-empty variants) | codegen_enum_union_two_payloads_layout | PASS |
| E::C(42) construction (i64 at field 2) | codegen_enum_union_variant_c_construction | PASS |
| E::B(7) construction (i32 at field 1) | codegen_enum_union_variant_b_construction | PASS |
| `E::B(x) => x` extracts from field 1 | codegen_enum_union_match_b_extracts_payload | PASS |
| `E::C(x) => x` extracts from field 2 | codegen_enum_union_match_c_extracts_payload | PASS |
| `Opt::Some(x) => x` extracts payload (P0 fix) | codegen_enum_binding_extraction_case_b | PASS |
| Multi-field variant → 4-field flat layout | codegen_enum_union_multi_field_variant_layout | PASS |
| Mixed i32/f64 payloads | codegen_enum_union_mixed_types_layout | PASS |
| Case B regression (unchanged) | codegen_enum_union_regression_single_payload | PASS |
| Case A regression (unchanged) | codegen_enum_union_regression_all_unit | PASS |
| Struct variant pattern binding | codegen_enum_union_struct_variant_match | PASS |
| End-to-end: arm returns payload | codegen_enum_union_match_returns_correct_value | PASS |

**Stage 3.48 expected**: 12 | **Actual**: 12 | **Coverage**: 100%

### R15 audit coverage (30 cases)

| Group | Cases | Coverage |
|-------|-------|----------|
| Regression (R14 carry-forward) | r01-r08 (8) | PASS |
| L-ENUM-UNION + L-ENUM-BINDING | u01-u14 (14) | PASS |
| Edge cases (3 variants, bool, struct, return, param, wildcard, tuple, two-enums) | e01-e08 (8) | PASS |

**R15 expected**: 30 | **Actual**: 30 | **Coverage**: 100%

## Stage 3.47 — AdtLayout::Enum Coverage (L-PIPE-1 closure)

Stage 3.47 closed L-PIPE-1's enum extension (silently added in Stage 3.38).
Enum storage layouts are now resolved via `mir.adt_layouts` (populated by
MIR lower from HIR). The `AdtLayout::Enum` variant stores **all** variants'
payloads (Stage 3.48 now consumes all of them — flat layout).

The Stage 3.47 enum-related AdtLayout tests (covered by R14 audit `i04`, `i05`,
`i06`, `i13`, `i14`, `e03`, `e07`):

| Scenario | Audit Case | Status |
|----------|-----------|--------|
| Unit-only enum param = `{ i32 }` | i04 | PASS |
| Tuple-variant enum param = `{ i32, i32 }` | i05 | PASS |
| Enum match via AdtLayout discriminant | i06 | PASS |
| Enum struct-variant = `{ i32, i32, i32 }` | i13 | PASS |
| Multi-variant enum (Case C flat layout — Stage 3.48 update) | i14 | PASS |
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
