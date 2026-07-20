# Global Test Matrix

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.12 (§17)

## Current Status

| Stage | Tests | Coverage | Status |
|-------|-------|----------|--------|
| Stage 0 (lexer/parser/AST) | 245 | ~100% | ✅ Complete |
| Stage 1 (HIR/resolve) | 451 | ~100% | ✅ Complete |
| Stage 2 (MIR/typeck/borrowck) | 673 | ~100% | ✅ Complete |
| Stage 3 (codegen) | 814 | ~95% | 🔄 In progress |

## Stage 3 Test Breakdown

| Sub-stage | Feature | Tests | Status |
|-----------|---------|-------|--------|
| 3.1-3.4 | Basic codegen (return, arith, variables, control flow) | 36 | ✅ |
| 3.5-3.7 | Params, match, float, cast | 15 | ✅ |
| 3.21 | Typed aggregates | 10 | ✅ |
| 3.22 | Block-scoped cache | 6 | ✅ |
| 3.24 | Overflow checks | 8 | ✅ |
| 3.25 | Div-by-zero checks | 6 | ✅ |
| 3.27 | String literals | 13 | ✅ |
| 3.28 | Byte strings + u8/i8 | 9 | ✅ |
| 3.30 | ADT/struct codegen | 13 | ✅ |
| 3.32 | Field type resolution | 6 | ✅ |
| 3.34 | Field mutation | 8 | ✅ |
| 3.36 | Field type propagation | 8 | ✅ |
| 3.38 | Enum variant codegen | 10 | ✅ |
| 3.40 | Enum match | 8 | ✅ |
| **Total codegen** | | **141** | ✅ |
| Gate audits R1-R9 | Audit cases | 315 cumulative | ✅ |

## Deferred Items (≤5% allowed per §17.3)

| ID | Feature | Reason | Plan |
|----|---------|--------|------|
| L1 | PHI node optimization | Not correctness; optimization | Stage 4 |
| L3 | Closure codegen | New feature | Stage 4 |
| L5 | Trait dispatch | New feature | Stage 5 |
| L8 | lli execution verification | Env lacks LLVM tools | When available |
| L9 | i128/u128 | Simplified to i64 | Stage 4 |
| L10 | Float bitwise ops | Edge case | Stage 4 |
| L11 | Shift-count overflow | Edge case | Stage 4 |
| L13 | Fat pointers | Simplification | Stage 4 |
| L14 | i16/u16 → i32 | Simplification | Stage 4 |
| L15 | String-as-function-arg | Requires L13 | Stage 4 |
| L-ENUM-UNION | Enum union payload | Simplification | Stage 4 |
| L-COPY-ADT | Proper Copy trait | Needs TraitResolver | Stage 5 |
| L-PIPE-1 | HIR lookup for Adt storage | Per §16.2.1 allowed | Stage 4 |
