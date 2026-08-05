# Stage 16.73 — Test Plan: Where Clause Checking

> **Author**: redskaber
> **Date**: 2026-08-05
> **Version**: v0.259.0

## 1. Test Scope

Stage 16.73 implements where clause checking. Tests verify valid and
invalid where clause bounds.

## 2. Test File

- `src/typeck/where_clause.rs` — 5 unit tests
- All passing ✅

## 3. Unit Test Coverage (5 tests)

| # | Test | Description |
|---|------|-------------|
| 1 | `where_clause_valid_trait` | `where T: Clone` with existing trait → no error |
| 2 | `where_clause_unknown_trait` | `where T: NonExistentTrait` → error |
| 3 | `where_clause_on_struct` | `struct Foo<T> where T: Clone` → no error |
| 4 | `no_where_clause` | No where clause → no error |
| 5 | `where_clause_on_impl` | `impl<T> Foo for S<T> where T: Foo` → no error |
