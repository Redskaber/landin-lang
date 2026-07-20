# Codegen Basic Tests

> **Author**: redskaber
> **Corresponds to**: `tests/codegen_tests.rs` (Stage 3.1-3.7)
> **Cross-ref**: `docs/develop/v0/stage-3/dev-log.md` Stage 3.1-3.7

## Test Target

Verify basic LLVM IR codegen for: function definition, return, arithmetic,
variables, control flow, comparisons, borrow/deref, calls, params, match,
float, cast.

## Covered Scenarios

| Scenario | Test Function | Status |
|----------|--------------|--------|
| Constant return | codegen_return_constant | PASS |
| Addition | codegen_addition | PASS |
| Subtraction | codegen_subtraction | PASS |
| Multiplication | codegen_multiplication | PASS |
| Division | codegen_division | PASS |
| Negation | codegen_negation | PASS |
| Chained arithmetic | codegen_chained_arith | PASS |
| Let binding | codegen_let_binding | PASS |
| Function definition | codegen_function_definition | PASS |
| Multiple functions | codegen_multiple_functions | PASS |
| Let with arith | codegen_let_with_arith | PASS |
| Param passed | codegen_param_passed | PASS |
| Empty body | codegen_empty_body | PASS |
| Equality | codegen_equality | PASS |
| Less than | codegen_less_than | PASS |
| Greater than | codegen_greater_than | PASS |
| Zext | codegen_zext | PASS |
| Borrow + deref | codegen_borrow_deref | PASS |
| If-else | codegen_if_else | PASS |
| While loop | codegen_while_loop | PASS |
| Function call | codegen_function_call | PASS |
| Function with params | codegen_function_with_params | PASS |
| Params stored to allocas | codegen_params_stored_to_allocas | PASS |
| Call with args | codegen_call_with_args | PASS |
| Call with multiple args | codegen_call_with_multiple_args | PASS |
| Recursive fibonacci | codegen_recursive_fibonacci | PASS |
| Match int | codegen_match_int | PASS |
| Match default | codegen_match_default | PASS |
| Float constant | codegen_float_constant | PASS |
| Float arith | codegen_float_arith | PASS |
| Recursive fibonacci full | codegen_recursive_fibonacci_full | PASS |
| Iterative sum full | codegen_iterative_sum_full | PASS |
| Borrow deref full | codegen_borrow_deref_full | PASS |
| Cast int to f64 | codegen_cast_int_to_f64 | PASS |
| Cast int to i64 | codegen_cast_int_to_i64 | PASS |
| Bool return | codegen_bool_return | PASS |

**Expected**: 36 | **Actual**: 36 | **Coverage**: 100%
