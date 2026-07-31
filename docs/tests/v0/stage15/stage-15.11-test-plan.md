# Stage 15.11 — Test Plan

> **Date**: 2026-07-31
> **Version**: v0.136.0 → v0.137.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3

## 1. Test Scope

Stage 15.11 changes `Const.ty` from `Box<Ty>` to `Ty`. This affects all
Const construction and consumption sites.

| Area | Test type | Count |
|------|-----------|-------|
| Const.ty Ty construction | Integration | 7 new |
| Regression (existing tests) | All existing | 1983 + 5216 |

## 2. Integration Test Module

**Path**: `tests/v0/stage15/plan/const_ty_box_to_ty_tests.rs`
**Registered as**: `stage15_const_ty_box_to_ty_tests`

### 2.1 Test cases

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_11_integer_constant` | Integer Const construction |
| 2 | `stage15_11_boolean_constant` | Boolean Const construction |
| 3 | `stage15_11_function_call_with_constant` | Function call with Const arg |
| 4 | `stage15_11_binary_op_with_constants` | Binary op with Const operands |
| 5 | `stage15_11_array_with_constants` | Array with Const elements |
| 6 | `stage15_11_method_call_with_constant` | Method call with Const |
| 7 | `stage15_11_match_with_constant` | Match with Const discriminant |

## 3. Regression Test Strategy

### 3.1 Updated source files

8 source files updated to use `ty: X` instead of `ty: Box::new(X)` for
Const construction. These changes are transparent — the Const struct
still has the same fields, just `ty` is `Ty` instead of `Box<Ty>`.

### 3.2 Updated test files

1 test file updated for Const construction. All existing tests must
continue to pass.

### 3.3 Conformance tests

All 5216 conformance tests must continue to pass. The Const.ty type change
is transparent at the user-facing level — `compile()` and codegen output
are unchanged.
