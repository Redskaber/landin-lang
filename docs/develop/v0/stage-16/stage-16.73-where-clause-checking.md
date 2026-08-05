# Stage 16.73 — Where Clause Checking

> **Author**: redskaber
> **Date**: 2026-08-05
> **Version**: v0.258.0 → v0.259.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.73 implements where clause checking — verifying that where clause
bounds reference valid traits. This is the first step toward full where
clause semantic checking.

**New module**: `src/typeck/where_clause.rs`
- `check_where_clauses(hir, resolver, interner)` — checks all where clauses in crate
- `check_where_clause_for_generics(generics, item_name, ...)` — checks one HirGenerics
- 5 unit tests covering valid/unknown/no-where-clause cases

**Driver integration**: Called after object safety check, before DynTraitMIRPlan.

**Current scope**: Verifies trait existence (Res::Def vs Res::Unknown/Err).
Full semantic checking (does the type implement the trait?) is deferred.

**Test results**: 8111 tests passing (358 lib + 2529 integration + 5224
conformance), 0 failures, 0 warnings. +5 new unit tests.

## 2. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 358/358 PASS (+5 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2529/2529 PASS
- **Total: 8111 tests passing, 0 failures, 0 warnings.**
