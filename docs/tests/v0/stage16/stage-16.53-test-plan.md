# Stage 16.53 — Test Plan: Type Substitution

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.239.0

## 1. Test Scope

Stage 16.53 implements the `substitute(ty, substs)` function and integrates
it into field type resolution. Tests verify:

1. The `substitute` function correctly replaces `Param` with concrete types
2. `substitute` handles all `TyKind` variants (leaf, recursive, generic)
3. Generic struct field access produces substituted concrete types
4. Generic struct literals compile with correct field types
5. Generic enum variants work with substitution
6. No regressions on non-generic code
7. MIR inspection confirms substs are propagated and substituted

## 2. Test Files

- `src/mir/substitute.rs` — 29 unit tests (pure function tests)
- `tests/v0/stage16/plan/stage16_53_substitute_tests.rs` — 18 integration tests
- All passing ✅

## 3. Unit Test Coverage (29 tests in `src/mir/substitute.rs`)

### §1. Leaf types — no substitution needed (5 tests)
| # | Test | Description |
|---|------|-------------|
| 1 | `substitute_leaf_bool` | Bool unchanged |
| 2 | `substitute_leaf_int` | Int unchanged |
| 3 | `substitute_leaf_str` | Str unchanged |
| 4 | `substitute_leaf_never` | Never unchanged |
| 5 | `substitute_leaf_error` | Error unchanged |

### §2. Param — the core replacement (4 tests)
| # | Test | Description |
|---|------|-------------|
| 6 | `substitute_param_replaced` | Param(0) + [i32] → i32 |
| 7 | `substitute_param_second_index` | Param(1) + [i32, bool] → bool |
| 8 | `substitute_param_out_of_bounds` | Param(5) + [i32] → Param(5) (unchanged) |
| 9 | `substitute_param_empty_substs` | Param(0) + [] → Param(0) (unchanged) |

### §3. Ref — substitute inner (1 test)
| # | Test | Description |
|---|------|-------------|
| 10 | `substitute_ref` | &Param(0) + [i32] → &i32 |

### §4. RawPtr — substitute inner (1 test)
| # | Test | Description |
|---|------|-------------|
| 11 | `substitute_raw_ptr` | *mut Param(0) + [i32] → *mut i32 |

### §5. Array — substitute inner element type (1 test)
| # | Test | Description |
|---|------|-------------|
| 12 | `substitute_array` | [Param(0); 10] + [i32] → [i32; 10] |

### §6. Slice — substitute inner (1 test)
| # | Test | Description |
|---|------|-------------|
| 13 | `substitute_slice` | [Param(0)] + [i32] → [i32] |

### §7. Tuple — substitute each element (1 test)
| # | Test | Description |
|---|------|-------------|
| 14 | `substitute_tuple` | (Param(0), Param(1), i32) + [bool, u64] → (bool, u64, i32) |

### §8. Adt — substitute inner substs (3 tests)
| # | Test | Description |
|---|------|-------------|
| 15 | `substitute_adt` | Adt(1, [Param(0)]) + [i32] → Adt(1, [i32]) |
| 16 | `substitute_adt_multiple_substs` | Adt(2, [Param(0), Param(1)]) + [i32, bool] → Adt(2, [i32, bool]) |
| 17 | `substitute_adt_empty_substs` | Adt(3, []) + [i32] → Adt(3, []) |

### §9. FnDef — substitute inner substs (1 test)
| # | Test | Description |
|---|------|-------------|
| 18 | `substitute_fn_def` | FnDef(5, [Param(0)]) + [i32] → FnDef(5, [i32]) |

### §10. Closure — substitute inner substs (1 test)
| # | Test | Description |
|---|------|-------------|
| 19 | `substitute_closure` | Closure(7, [Param(0)]) + [i32] → Closure(7, [i32]) |

### §11. FnPtr — substitute inputs + output (1 test)
| # | Test | Description |
|---|------|-------------|
| 20 | `substitute_fn_ptr` | fn(Param(0)) -> Param(0) + [i32] → fn(i32) -> i32 |

### §12. Infer — not substituted (1 test)
| # | Test | Description |
|---|------|-------------|
| 21 | `substitute_infer` | Infer(TyVar(42)) unchanged |

### §13. Nested types — deep substitution (3 tests)
| # | Test | Description |
|---|------|-------------|
| 22 | `substitute_nested_adt` | Vec<Vec<T>> + [i32] → Vec<Vec<i32>> |
| 23 | `substitute_nested_ref_adt` | &Box<T> + [i32] → &Box<i32> |
| 24 | `substitute_tuple_of_params` | (T, T, U) + [i32, bool] → (i32, i32, bool) |

### §14. substitute_substs — substitute a substs slice (3 tests)
| # | Test | Description |
|---|------|-------------|
| 25 | `substitute_substs_basic` | [Param(0), Param(1)] + [i32, bool] → [i32, bool] |
| 26 | `substitute_substs_empty` | [] + [i32] → [] |
| 27 | `substitute_substs_no_params` | [i32, bool] + [f64] → [i32, bool] (unchanged) |

### §15. Idempotency — empty substs is a no-op (2 tests)
| # | Test | Description |
|---|------|-------------|
| 28 | `substitute_empty_substs_idempotent` | i32 + [] → i32 |
| 29 | `substitute_empty_substs_on_adt` | Adt(1, [i32]) + [] → Adt(1, [i32]) |

## 4. Integration Test Coverage (18 tests)

### §1. substitute function — pure unit tests (3 tests)
| # | Test | Description |
|---|------|-------------|
| 1 | `substitute_param_replacement` | Param(0) + [i32] → i32 |
| 2 | `substitute_leaf_noop` | i32 + [bool] → i32 |
| 3 | `substitute_substs_slice` | [Param(0), Param(1)] + [i32, bool] → [i32, bool] |

### §2. Generic struct field access — end-to-end (4 tests)
| # | Test | Description |
|---|------|-------------|
| 4 | `generic_struct_field_access_compiles` | `Box<i32> { val: 42 }; b.val` |
| 5 | `generic_struct_two_params_field_access` | `Pair<i32, i32> { a: 1, b: 2 }; p.a + p.b` |
| 6 | `generic_struct_field_in_method` | `impl<X> S<X> { fn get(&self) -> X { self.x } }` |
| 7 | `generic_struct_trait_impl_method_call` | `impl<X: T> T for S<X> { fn f(&self) -> i32 { self.x.f() } }` |

### §3. Generic enum — end-to-end (2 tests)
| # | Test | Description |
|---|------|-------------|
| 8 | `generic_enum_match` | `Opt<i32>` with match |
| 9 | `generic_enum_unit_variant` | `Opt::None` with `Opt<i32>` annot |

### §4. No regressions (3 tests)
| # | Test | Description |
|---|------|-------------|
| 10 | `non_generic_struct_no_regression` | Point struct still works |
| 11 | `non_generic_struct_method_no_regression` | Point with method still works |
| 12 | `non_generic_enum_no_regression` | Color enum still works |

### §5. MIR inspection (2 tests)
| # | Test | Description |
|---|------|-------------|
| 13 | `generic_struct_local_has_substs` | Local decl has `Adt(def, [i32])` |
| 14 | `generic_field_access_produces_concrete_type` | `b.val` produces `i32` local |

### §6. Complex generic patterns (4 tests)
| # | Test | Description |
|---|------|-------------|
| 15 | `nested_generic_struct` | `Box<Box<i32>>` |
| 16 | `generic_struct_tuple_field` | `Pair<T> { val: (T, T) }` |
| 17 | `generic_struct_ref_field` | `RefBox<T> { val: &T }` |
| 18 | `multiple_generic_structs` | Box + Pair in same program |

## 5. Test Strategy

### 5.1 Pure Function Tests (Unit, 29 tests)

Tests in `src/mir/substitute.rs` use hand-crafted `Ty` values with `Param`
placeholders. They verify the `substitute` function's correctness in
isolation — no HIR, no compilation, no side effects.

### 5.2 End-to-End Compilation Tests (Integration, 15 tests)

Tests in `stage16_53_substitute_tests.rs` use the public `compile(src)`
API and check `result.has_errors()`. They verify that generic struct/enum
construction and field access compile successfully.

### 5.3 MIR Inspection Tests (Integration, 2 tests)

Tests 13-14 go beyond black-box compilation by inspecting the resulting
`MirBody` to verify:
- Local declarations carry substs (test 13)
- Field access produces concrete types (test 14)

## 6. Conformance Suite

All 5224 conformance tests pass — no regressions on:
- 01-typecheck/01-trait-resolution (including the previously-failing
  `020-generic-trait-impl.lin` and `038-trait-trait-impl-with-where-clause.lin`)
- 04-e2e/03-closures (28 tests)
- 04-e2e/04-error-handling (28 tests)
- 04-e2e/06-run-ok (171 tests)

## 7. References

- Stage 16.53 design: `docs/develop/v0/stage-16/stage-16.53-type-substitution.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Stage 16.52 test plan: `docs/tests/v0/stage16/stage-16.52-test-plan.md`
