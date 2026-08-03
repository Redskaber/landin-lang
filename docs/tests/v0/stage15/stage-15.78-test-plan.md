# Stage 15.78 — Test Plan: Array Length Unify Fix + Error Test Audit

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.202.0 → v0.203.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.78 changes one arm in `src/typeck/unify.rs::unify_resolved`:
the `TyKind::Array` arm now compares the length `Const` values, not
just the element types. The change is small but has soundness
implications — previously-accepted programs with mismatched array
lengths now correctly fail to compile.

Additionally, this stage audits the 416 `EXPECTED: compile_error`
tests in `tests/conformance/` per the user's directive.

## 2. New Unit Tests (3 tests)

Added to `src/typeck/unify.rs::tests`:

### 2.1 `unify_array_same_length`

```rust
let a = Array(I32, Const(Uint(3)));
let b = Array(I32, Const(Uint(3)));
assert!(unify(&a, &b).is_ok());
```

Tests that same-length arrays with same element type unify
successfully.

### 2.2 `unify_array_different_length`

```rust
let a = Array(I32, Const(Uint(3)));
let b = Array(I32, Const(Uint(2)));
assert!(unify(&a, &b).is_err());
```

Tests the new soundness check: different-length arrays fail to unify.

### 2.3 `unify_array_unevaluated_length_lenient`

```rust
let a = Array(I32, Const(Uint(3)));
let b = Array(I32, Const(Unevaluated));
assert!(unify(&a, &b).is_ok());
```

Tests the lenient fallback: when either length is `Unevaluated`,
unify succeeds (no false positives).

## 3. Conformance Test Changes (4 tests flipped)

All 4 tests flipped `compile_ok → compile_error`:

| # | Test | Code | Old Expected | New Expected | Reason |
|---|------|------|--------------|--------------|--------|
| 1 | `01-typecheck/99-error-cases/035-type-mismatch-array.lin` | `let arr: [i32; 3] = [1, 2];` | `compile_ok` | `compile_error` | Array length mismatch (3 vs 2) now caught |
| 2 | `01-typecheck/99-error-cases/043-err-mismatch-array-sizes.lin` | `let x: [i32; 3] = [1, 2];` | `compile_ok` | `compile_error` | Same pattern |
| 3 | `01-typecheck/02-generics/064-gen-generic-with-container-pattern.lin` | `Container { data: [], len: 0 }` (field type `[T; 10]`) | `compile_ok` | `compile_error` | `[]` length 0 ≠ field length 10 |
| 4 | `06-stdlib/01-alloc/025-alloc-vec-wrapper.lin` | `Vec<T>{data:[],len:0}` (field type `[T; 10]`) | `compile_ok` | `compile_error` | Same pattern |

Tests 1 and 2 are "Stage 0 limitation — typeck does not catch this"
tests that the new soundness fix correctly catches.

Tests 3 and 4 are "Stage 0 limitation" tests for the empty-array-as-
field-initializer pattern. The new soundness check correctly rejects
these (length 0 ≠ length 10). Supporting `data: []` properly requires
an "empty array with inferred length" feature (deferred to a future
stage — see §7.1 of stage doc).

## 4. Pipeline Path Coverage (§17.5.1)

### 4.1 Array literal path (unchanged behavior for same-length)

| Stage | Path | Test |
|-------|------|------|
| Lexer | `[`, `]`, `,` tokens | ✅ lexer_tests |
| Parser | `HirExprKind::Array` | ✅ parser_tests |
| HIR lower | `HirExprKind::Array` | ✅ hir_lower_tests |
| MIR lower | `Rvalue::Aggregate(Array, ...)` + concrete `Array(elem_ty, Const(N))` type | ✅ mir_lower_tests |
| Typeck | `unify_resolved(Array(I32, 3), Array(I32, 3))` → OK | ✅ `unify_array_same_length` |
| Borrowck | borrow of `Array` | ✅ borrowck_tests |
| Codegen | `Rvalue::Aggregate` → LLVM array type | ✅ codegen_tests |

### 4.2 Array length mismatch path (new soundness check)

| Stage | Path | Test |
|-------|------|------|
| Lexer | (same as 4.1) | ✅ |
| Parser | (same as 4.1) | ✅ |
| HIR lower | (same as 4.1) | ✅ |
| MIR lower | `Rvalue::Aggregate(Array, [op1, op2])` + `Array(Infer(TyVar), Const(2))` | ✅ |
| Typeck | `unify_resolved(Array(I32, 3), Array(I32, 2))` → ERR | ✅ `unify_array_different_length` + 4 conformance tests |
| (borrowck not reached — typeck error) | — | — |
| (codegen not reached — typeck error) | — | — |

## 5. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Existing array-using programs break | LOW-MEDIUM | All 5206 same-length array tests still pass; only 4 length-mismatch tests fail (intentional) |
| Tuple unify regressions | LOW | Tuple arm unchanged; only Array arm modified |
| Codegen regressions | LOW | No codegen changes |
| False positives on Unevaluated lengths | LOW | Lenient fallback (skip length check if either side is Unevaluated) |

**Overall risk**: LOW. The change is localized to one `match` arm in
`unify_resolved`. The lenient fallback ensures no false positives for
symbolic array lengths.

## 6. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 224/224 PASS | ✅ 224/224 PASS (was 221, +3 new) |
| `cargo test --features llvm-backend --test all_tests` | 2130/2130 PASS | ✅ 2130/2130 PASS |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS (4 flipped, 0 added) |
| Conformance test count | unchanged (5216) | ✅ 5216 |

## 7. Test Sign-off

- ✅ All 224 lib tests pass (was 221, +3 new unify tests)
- ✅ All 2130 integration tests pass
- ✅ All 5216 conformance tests pass
- ✅ 4 conformance tests correctly flipped (soundness fix)
- ✅ 0 clippy warnings
- ✅ fmt clean

**Stage 15.78 PASSED**.
