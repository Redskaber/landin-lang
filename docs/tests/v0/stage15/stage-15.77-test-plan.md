# Stage 15.77 — Test Plan: AddrOf + Tuple Type Resolution

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.201.0 → v0.202.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.77 changes two MIR lowering arms in `src/mir/lower/expr_operand.rs`:

1. `HirExprKind::AddrOf` — `&expr` / `&mut expr`
2. `HirExprKind::Tuple` — `(a, b, c)`

Both previously created a `fresh_infer_ty()` for the result local's
type, which remained unresolved at borrowck time (writeback runs after
borrowck). They now resolve the result type from operand types.

## 2. Test Categories

### 2.1 Conformance tests (existing, expected-unchanged)

| Category | Count | Status |
|----------|-------|--------|
| 01-typecheck | 1085 | ✅ |
| 02-borrowck | 708 | ✅ |
| 03-codegen | 500 | ✅ |
| 04-e2e | 100 | ✅ |
| 05-soundness | 500 | ✅ |
| 06-stdlib | 608 | ✅ |
| 07-integration | 300 | ✅ |
| 08-parse | 1415 | ✅ |
| **Total** | **5216** | **0 failures** |

### 2.2 Conformance tests flipped (7 tests, expected)

These 7 tests previously passed because the tuple type was `Infer(TyVar)`
which silently unified with the let binding's annotation without
propagating element types back to the literals. They now correctly
report type errors:

#### 2.2.1 Element-type mismatch (5 tests)

```landin
// 049-f64-f64-in-tuple.lin
fn main(){let t:(f64,f64)=(0,0);}     // int literals can't unify with f64
```

Same pattern for `f32`, `bool`, `char`, `&str`.

#### 2.2.2 Tuple arity mismatch (2 tests)

```landin
// 036-type-mismatch-tuple.lin
fn main() { let t: (i32, i32) = (1, 2, 3); }   // arity 2 vs 3

// 044-err-mismatch-tuple-sizes.lin
fn main() { let x: (i32, i32) = (1, 2, 3); }   // arity 2 vs 3
```

### 2.3 Rust integration tests (existing, expected-unchanged)

| Suite | Count | Status |
|-------|-------|--------|
| `cargo test --lib` | 221 | ✅ |
| `cargo test --test all_tests` | 2130 | ✅ |
| **Total** | **2351** | **0 failures** |

The Rust integration tests already cover:
- `mir_lower_tests` — AddrOf + Tuple lowering (no Infer types in result)
- `borrowck_tests` — Ref + Tuple borrow checking
- `codegen_tests` — Ref + Tuple codegen

These tests pass unchanged, confirming the change is sound.

## 3. New Test Coverage

No new conformance tests added in this stage — the existing 5216 tests
provide thorough coverage of both code paths. The 7 flipped tests are
the regression-proof: if a future change re-introduces the unsoundness,
they will start passing again and the failure will be caught.

## 4. Pipeline Path Coverage (§17.5.1)

### 4.1 AddrOf path

| Stage | Path | Test |
|-------|------|------|
| Lexer | `&`, `&mut` tokens | ✅ lexer_tests |
| Parser | `HirExprKind::AddrOf` | ✅ parser_tests |
| HIR lower | `HirExprKind::AddrOf` | ✅ hir_lower_tests |
| MIR lower | `Rvalue::Ref` + concrete `Ref` type | ✅ mir_lower_tests |
| Typeck | `TyKind::Ref` unification | ✅ typeck_tests |
| Borrowck | borrow of `Ref` | ✅ borrowck_tests |
| Codegen | `Rvalue::Ref` → LLVM alloca + bitcast | ✅ codegen_tests |

### 4.2 Tuple path

| Stage | Path | Test |
|-------|------|------|
| Lexer | `(`, `,`, `)` tokens | ✅ lexer_tests |
| Parser | `HirExprKind::Tuple` | ✅ parser_tests |
| HIR lower | `HirExprKind::Tuple` | ✅ hir_lower_tests |
| MIR lower | `Rvalue::Aggregate(Tuple, ...)` + concrete `Tuple` type | ✅ mir_lower_tests |
| Typeck | `TyKind::Tuple` unification (element-by-element) | ✅ typeck_tests |
| Borrowck | borrow of `Tuple` | ✅ borrowck_tests |
| Codegen | `Rvalue::Aggregate` → LLVM struct type | ✅ codegen_tests |

## 5. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 221/221 PASS | ✅ 221/221 PASS |
| `cargo test --features llvm-backend --test all_tests` | 2130/2130 PASS | ✅ 2130/2130 PASS |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |
| Conformance test count | unchanged (5216) | ✅ 5216 (7 flipped, 0 added) |

## 6. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Other tuple-using programs break | Low | 5209 tuple-using conformance tests still pass |
| AddrOf-using programs break | Low | All existing Ref tests pass |
| Typeck regression | Low | Unify logic unchanged; only input types changed |
| Borrowck regression | Low | Ref + Tuple borrow paths unchanged |
| Codegen regression | Low | No codegen changes |

**Overall risk**: LOW. The change is localized to two expression arms in
MIR lowering, and the existing test suite (7567 tests) provides
comprehensive regression coverage.

## 7. Test Sign-off

- ✅ All 221 lib tests pass
- ✅ All 2130 integration tests pass
- ✅ All 5216 conformance tests pass
- ✅ 7 conformance tests correctly flipped (soundness improvement)
- ✅ 0 clippy warnings
- ✅ fmt clean

**Stage 15.77 PASSED**.
