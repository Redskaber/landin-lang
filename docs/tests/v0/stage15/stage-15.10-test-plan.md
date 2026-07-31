# Stage 15.10 — Test Plan

> **Date**: 2026-07-31
> **Version**: v0.135.0 → v0.136.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3

## 1. Test Scope

Stage 15.10 changes `SubstsRef` from `Vec<Ty>` to `Rc<[Ty]>`. This affects
all `TyKind::Adt`, `TyKind::FnDef`, `TyKind::Closure` construction and
consumption sites.

| Area | Test type | Count |
|------|-----------|-------|
| SubstsRef Rc<[Ty]> construction | Integration | 7 new |
| Regression (existing tests) | All existing | 1976 + 5216 |

## 2. Integration Test Module

**Path**: `tests/v0/stage15/plan/substs_ref_rc_tests.rs`
**Registered as**: `stage15_substs_ref_rc_tests`

### 2.1 Test cases

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_10_struct_empty_substs` | Struct construction with empty Rc<[Ty]> |
| 2 | `stage15_10_enum_empty_substs` | Enum construction with empty Rc<[Ty]> |
| 3 | `stage15_10_closure_with_captures` | Closure construction with capture substs |
| 4 | `stage15_10_method_call_on_adt` | Method call on Adt-typed local (writeback) |
| 5 | `stage15_10_nested_struct_access` | Nested struct access (substs propagate) |
| 6 | `stage15_10_closure_capturing_struct` | Closure capturing struct (Adt in substs) |
| 7 | `stage15_10_multiple_closures` | Multiple closures with different captures |

## 3. Regression Test Strategy

### 3.1 Updated test files

4 test files were updated to use `vec![].into()` instead of `vec![]` for
TyKind::Adt construction. These tests verify the Rc<[Ty]> construction
works correctly in unit-test contexts.

### 3.2 Conformance tests

All 5216 conformance tests must continue to pass. The SubstsRef type change
is transparent at the user-facing level — `compile()` and codegen output
are unchanged.

## 4. Coverage Matrix

| Module | Unit tests | Integration tests | Conformance |
|--------|-----------|-------------------|-------------|
| `SubstsRef: Rc<[Ty]>` | Existing ty tests | 7 new | 5216 (all) |
| Writeback mutation (rebuild pattern) | Existing writeback tests | 2 (method call + closure) | All pass |
| AdtLayouts with Rc substs | Existing adt_layout tests | 1 (nested struct) | All pass |
