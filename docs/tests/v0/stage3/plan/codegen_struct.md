# Codegen Struct/ADT Tests

> **Author**: redskaber
> **Corresponds to**: `tests/codegen_tests.rs` (Stage 3.21-3.47)
> **Cross-ref**: `docs/develop/v0/stage-3/dev-log.md` Stage 3.21-3.47

## Test Target

Verify struct/ADT codegen: typed aggregates, block-scoped cache, struct
construction, field access, field mutation, field type resolution, field
type propagation through arithmetic, and (Stage 3.47) **AdtLayout
side-table**-driven type resolution per §16 (no codegen→HIR lookup).

## Stage 3.47 — AdtLayout Side-Table Coverage (L-PIPE-1 closure)

Stage 3.47 closed L-PIPE-1 by introducing `AdtLayout` / `AdtLayouts` on
`MirBody`. Codegen now resolves `TyKind::Adt(def_id, _)` storage layouts
via `mir.adt_layouts` (populated by MIR lower) instead of reading HIR.
The following 10 new tests verify this:

| Scenario | Test Function | Status |
|----------|--------------|--------|
| Struct param from AdtLayout | codegen_adt_layout_struct_param | PASS |
| Struct return from AdtLayout | codegen_adt_layout_struct_return | PASS |
| Struct local alloca from AdtLayout | codegen_adt_layout_struct_local_alloca | PASS |
| Struct with i128 field (no width regression) | codegen_adt_layout_struct_with_i128_field | PASS |
| Struct with &str field (no codegen→MIR-lower call) | codegen_adt_layout_struct_with_ref_field | PASS |
| Nested struct AdtLayout recursion | codegen_adt_layout_nested_struct | PASS |
| Two distinct Adts in one fn | codegen_adt_layout_two_structs_in_one_fn | PASS |
| Tuple struct ctor uses AdtLayout | codegen_adt_layout_no_hir_lookup_in_codegen | PASS |
| (MirBody AdtLayout init) | mir_body_adt_layouts_starts_empty | PASS |
| (MirBody AdtLayout struct register) | mir_body_register_adt_layout_struct | PASS |
| (MirBody AdtLayout enum register) | mir_body_register_adt_layout_enum | PASS |
| (MirBody AdtLayout idempotent) | mir_body_register_adt_layout_idempotent | PASS |

**Stage 3.47 expected**: 14 | **Actual**: 14 | **Coverage**: 100%

## Covered Scenarios (Stage 3.21-3.36)

| Scenario | Test Function | Status |
|----------|--------------|--------|
| Tuple of mixed types | codegen_tuple_of_two_distinct_types | PASS |
| Tuple of three types | codegen_tuple_of_three_types | PASS |
| Array of i32 correct length | codegen_array_of_i32_correct_length | PASS |
| Array of i64 correct length | codegen_array_of_i64_correct_length | PASS |
| Typed call args i64 | codegen_typed_call_args_i64 | PASS |
| Typed call args mixed | codegen_typed_call_args_mixed | PASS |
| Typed ptr to i32 | codegen_typed_ptr_to_i32 | PASS |
| Array GEP uses correct type | codegen_array_gep_uses_correct_type | PASS |
| Tuple field access via GEP | codegen_tuple_field_access_via_gep | PASS |
| Insertvalue with typed field | codegen_insertvalue_with_typed_field | PASS |
| If-else merge correctness | codegen_if_else_merge_correct_value | PASS |
| If-else stores to result slot | codegen_if_else_stores_to_result_slot | PASS |
| Nested if correctness | codegen_nested_if_correctness | PASS |
| Match correctness | codegen_match_correctness | PASS |
| While loop correctness | codegen_while_loop_correctness | PASS |
| If-else with arith | codegen_if_else_with_arith | PASS |
| Named struct construction | codegen_named_struct_construction | PASS |
| Named struct field access | codegen_named_struct_field_access | PASS |
| Named struct alloca | codegen_named_struct_alloca_has_struct_type | PASS |
| Tuple struct construction | codegen_tuple_struct_construction | PASS |
| Tuple struct field access | codegen_tuple_struct_field_access | PASS |
| Tuple struct no fake function | codegen_tuple_struct_no_fake_function | PASS |
| Struct with mixed field types | codegen_struct_with_mixed_field_types | PASS |
| Struct returned from function | codegen_struct_returned_from_function | PASS |
| Struct passed to function | codegen_struct_passed_to_function | PASS |
| Unit struct | codegen_unit_struct | PASS |
| Empty struct | codegen_empty_struct | PASS |
| Struct field mutation | codegen_struct_field_mutation | PASS |
| Multiple structs distinct | codegen_multiple_structs_distinct | PASS |
| Field load i64 | codegen_field_load_correct_type_i64 | PASS |
| Field load f64 | codegen_field_load_correct_type_f64 | PASS |
| Field load bool | codegen_field_load_correct_type_bool | PASS |
| Field load u8 | codegen_field_load_correct_type_u8 | PASS |
| Field in arithmetic | codegen_field_in_arithmetic | PASS |
| Named field y | codegen_named_field_load_correct_type | PASS |
| Field chained access | codegen_field_chained_access | PASS |
| Field mutation works | codegen_field_mutation_works | PASS |
| Field mutation persists | codegen_field_mutation_persists | PASS |
| Named field mutation | codegen_field_mutation_named_field | PASS |
| i32 field mutation | codegen_field_mutation_i32 | PASS |
| Multiple field mutations | codegen_multiple_field_mutations | PASS |
| Local assignment regression | codegen_local_assignment_still_works | PASS |
| Field mutation in loop | codegen_field_mutation_in_loop | PASS |
| Field mutation correct GEP | codegen_field_mutation_correct_gep | PASS |
| Mutation overwrites initial | codegen_field_mutation_then_read | PASS |
| Field arithmetic i64 | codegen_field_arithmetic_uses_i64 | PASS |
| Field overflow i64 | codegen_field_arithmetic_overflow_check_i64 | PASS |
| Field arithmetic f64 | codegen_field_arithmetic_f64 | PASS |
| Field subtraction i64 | codegen_field_subtraction_i64 | PASS |
| Field multiplication i64 | codegen_field_multiplication_i64 | PASS |
| Two fields arithmetic | codegen_two_fields_arithmetic | PASS |
| Field division i64 | codegen_field_div_i64 | PASS |
| Field rem i64 | codegen_field_rem_i64 | PASS |
| Field arith i32 regression | codegen_field_arith_i32 | PASS |
| Chained field arith | codegen_field_arithmetic_chained | PASS |

**Stage 3.21-3.36 expected**: 55 | **Actual**: 55 | **Coverage**: 100%

## Cumulative Coverage

| Stage | Tests | Coverage |
|-------|-------|----------|
| 3.21-3.36 (typed aggregates + field ops) | 55 | 100% |
| 3.47 (AdtLayout side-table, L-PIPE-1) | 14 | 100% |
| **Total struct/ADT codegen tests** | **69** | **100%** |
