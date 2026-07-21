# Global Test Matrix

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.13 (§17 + §18)

## Current Status

| Stage | Tests | Coverage | Status |
|-------|-------|----------|--------|
| Stage 0 (lexer/parser/AST) | 245 | ~100% | ✅ Complete |
| Stage 1 (HIR/resolve) | 451 | ~100% | ✅ Complete |
| Stage 2 (MIR/typeck/borrowck) | 673 | ~100% | ✅ Complete |
| Stage 3 (codegen) | 938 | ~99% | 🔄 In progress |

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
| 3.42 | &str type fix | 6 | ✅ |
| 3.43 | Shift overflow check | 8 | ✅ |
| 3.44 | Const/Static value resolution | 8 | ✅ |
| 3.45 | L10 float bitwise ops via cast | 6 | ✅ |
| 3.46 | L14 + L9 full integer types (i8/i16/i32/i64/i128/usize/isize) | 13 | ✅ |
| 3.47 | L-PIPE-1 closure via AdtLayout side-table on MirBody (per §16) | 14 | ✅ |
| 3.48 | L-ENUM-UNION + L-ENUM-BINDING closure: flat enum storage + pattern binding extraction | 12 | ✅ |
| 3.49 | L13 fat pointer closure: &str/&[T] now { ptr, len } struct, not thin pointer | 12 | ✅ |
| 3.50 | Byte string fat pointer fix + comparison pointee type fix (Stage 3.49 latent bugs) | 10 | ✅ |
| 3.51 | Slice indexing fix: fat pointer data pointer dereference (Stage 3.49 latent P0) | 9 | ✅ |
| 3.52 | Slice element type propagation: load/store/arith use correct element type from fat pointer | 9 | ✅ |
| 3.53 | &str indexing element type fix: u8 element, not i32 (Stage 3.52 latent) | 9 | ✅ |
| 3.54 | Slice/array field store + detect_lvalue_storage_type Field projection fix | 9 | ✅ |
| **Total codegen** | | **266** | ✅ |
| Gate audits R1-R21 | Audit cases | 650 cumulative | ✅ |

## Deferred Items (≤5% allowed per §17.3)

| ID | Feature | Reason | Plan |
|----|---------|--------|------|
| L1 | PHI node optimization | Not correctness; optimization | Stage 4 |
| L3 | Closure codegen | New feature | Stage 4 |
| L5 | Trait dispatch | New feature | Stage 5 |
| L8 | lli execution verification | Env lacks LLVM tools | When available |
| ~~L9~~ | ~~i128/u128~~ | CLOSED in Stage 3.46 ✅ |
| ~~L10~~ | ~~Float bitwise ops~~ | CLOSED in Stage 3.45 ✅ |
| ~~L11~~ | ~~Shift-count overflow~~ | CLOSED in Stage 3.43 ✅ |
| ~~L13~~ | ~~Fat pointers~~ | CLOSED in Stage 3.49 ✅ |
| ~~L14~~ | ~~i16/u16 → i32~~ | CLOSED in Stage 3.46 ✅ |
| ~~L15~~ | ~~String-as-function-arg~~ | CLOSED in Stage 3.42 ✅ |
| ~~L-ENUM-UNION~~ | ~~Enum union payload~~ | CLOSED in Stage 3.48 ✅ |
| L-COPY-ADT | Proper Copy trait | Needs TraitResolver | Stage 5 |
| ~~L-PIPE-1~~ | ~~HIR lookup for Adt storage~~ | CLOSED in Stage 3.47 ✅ |
