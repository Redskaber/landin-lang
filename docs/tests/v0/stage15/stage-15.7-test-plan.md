# Stage 15.7 — Test Plan

> **Date**: 2026-07-31
> **Version**: v0.132.0 → v0.133.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3

## 1. Test Scope

Stage 15.7 consolidates 8 driver writeback passes into 2 functions. The
test plan covers both unit-level verification (synthetic MIR) and
integration-level verification (real HIR via `compile()`).

| Area | Test type | Count |
|------|-----------|-------|
| `writeback_type_propagation` (unit) | Rust unit (lib) | 5 new |
| `writeback_closures` (integration) | Rust integration (all_tests) | 7 new |
| Regression (existing features) | Conformance + Rust integration | 5216 + 1957 (unchanged) |

## 2. Unit Test Module

**Path**: `src/mir/lower/writeback.rs` (inline `#[cfg(test)] mod tests`)
**Purpose**: Verify writeback functions in isolation with synthetic MIR.

### 2.1 Test cases

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_7_tuple_aggregate_writeback` | Rule 1: `loc = (a, b)` → Tuple type |
| 2 | `stage15_7_local_copy_writeback` | Rule 5: `loc = Copy(src)` → src's type |
| 3 | `stage15_7_fixpoint_copy_chain` | Fixpoint convergence on `c = Copy(b); b = Copy(a)` |
| 4 | `stage15_7_no_writeback_when_concrete` | Concrete dest is NOT overwritten |
| 5 | `stage15_7_needs_writeback_helper` | Helper correctly identifies Infer/Error |

### 2.2 Test design rationale

Unit tests use synthetic MIR (manually constructed `MirBody` with known
local_decls and statements). This isolates the writeback logic from the
rest of the compiler, making failures easy to localize.

Each test constructs a minimal MirBody with 1 basic block, 1-2 statements,
and 2-3 local_decls. The writeback function is called, then the dest
local's type is asserted.

## 3. Integration Test Module

**Path**: `tests/v0/stage15/plan/writeback_consolidation_tests.rs`
**Registered as**: `stage15_writeback_consolidation_tests` in `tests/all_tests.rs`

### 3.1 Test cases

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_7_method_chain_writeback_integration` | Chained method calls (Call dest writeback) |
| 2 | `stage15_7_tuple_field_writeback_integration` | Tuple literal + field access |
| 3 | `stage15_7_array_index_writeback_integration` | Array indexing (Index projection) |
| 4 | `stage15_7_copy_chain_writeback_integration` | Copy/Move chain (fixpoint) |
| 5 | `stage15_7_closure_writeback_integration` | Closure with capture (3 sub-passes) |
| 6 | `stage15_7_struct_return_writeback_integration` | Struct-returning method call |
| 7 | `stage15_7_generic_method_no_hang_regression` | Generic method doesn't hang (bug fix) |

### 3.2 Test design rationale

Integration tests use `compile()` to run the full pipeline (lexer → parser
→ HIR → MIR lower → typeck → writeback). Each test compiles a small
Landin program and asserts no errors.

Test 7 is a regression test for the infinite-loop bug found during Stage
15.7. It verifies that a generic method call (which v0.1 doesn't support)
produces a compile error instead of hanging.

## 4. Regression Test Strategy

### 4.1 Conformance tests

All 5216 conformance tests must continue to pass. The writeback
consolidation is a pure refactoring (same behavior, different code
organization) — no test should observe different behavior.

Special attention to:
- `01-typecheck/02-generics/006-generic-method.lin` — was hanging before
  the convergence guard fix
- `04-e2e/06-run-ok/e2e-runok-026-array-repeat.lin` — was hanging
- `06-stdlib/02-std/015-clone-impl.lin` — was hanging

### 4.2 Rust integration tests

All 1957 existing integration tests must continue to pass. Run with:

```bash
cargo test --features llvm-backend
```

Expected: 1964 passed (1957 + 7 new), 0 failed, 2 ignored.

## 5. Coverage Matrix

| Module | Unit tests | Integration tests | Conformance |
|--------|-----------|-------------------|-------------|
| `writeback_type_propagation` | 5 (synthetic MIR) | 7 (real HIR) | 5216 (all) |
| `writeback_closures` | 0 (covered by integration) | 1 (closure test) | Existing closure tests |
| `driver.rs` (orchestration) | N/A | All pass | All pass |

## 6. Bug-Fix Verification

The infinite-loop bug was found by the conformance suite. The fix
(convergence guard) is verified by:

1. **Unit test**: `stage15_7_needs_writeback_helper` — confirms the
   `needs_writeback` predicate correctly identifies Infer/Error.
2. **Integration test**: `stage15_7_generic_method_no_hang_regression` —
   confirms the generic method case produces an error, not a hang.
3. **Conformance tests**: All 5 previously-hanging tests now pass.

## 7. Test File Location

Per §17.3 (test directory standardization):

```
tests/
└── v0/
    └── stage15/
        └── plan/
            ├── method_return_type_cache_tests.rs   (Stage 15.6)
            └── writeback_consolidation_tests.rs     # NEW (Stage 15.7)
```

The unit tests live inline in `src/mir/lower/writeback.rs` because they
test private helpers (`needs_writeback`, `compute_writeback_ty`) that
aren't accessible from the integration test directory.
