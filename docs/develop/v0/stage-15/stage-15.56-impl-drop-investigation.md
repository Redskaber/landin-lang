# Stage 15.56 — `impl Drop` Parser Support Investigation

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.181.0 → v0.182.0
> **Process**: stage-committee-process.md v3.23 §13.4 + §29
> **v0.2 Phase 3 Task 13 (step 2 of 5)**: `impl Drop` + RAII types

## 1. Executive Summary

Stage 15.56 investigates the parser support for `impl Drop for T`.
**Key finding**: The parser already supports `impl Drop for T { fn drop(&mut self) { ... } }`
— it was implemented as part of the general `impl Trait for T` parser
support (Stage 5.5). The `TraitResolver` also correctly collects Drop
impls via `is_drop_builtin`.

However, compiling a program with `impl Drop` causes a crash (exit code
137 — likely a segfault in codegen). This is because the drop glue
codegen (Stage 15.45) calls `drop_adt_<N>` which doesn't exist as an
emitted function — the drop glue function emission was not implemented.

**Conclusion**: The parser work for Stage 15.56 is **already done**. The
remaining work is **drop glue function emission** (Stage 15.57) — emitting
the `drop_adt_<N>` function that calls the user's `Drop::drop` method.

## 2. What Was Investigated

### 2.1 Parser support — ALREADY EXISTS

The parser's `parse_impl` function (`src/parser/items.rs:531`) handles
`impl Trait for Type { ... }` generically. It correctly:
1. Parses `impl Drop for Counter { ... }`.
2. Sets `of_trait = Some(Path("Drop"))`.
3. Parses `fn drop(&mut self) { ... }` as an impl item.
4. The HIR lower and resolver process it as a trait impl.

### 2.2 TraitResolver — ALREADY WORKS

`TraitResolver::is_drop_builtin(def_id, interner)` (Stage 5.10) checks
if a type implements `Drop` by looking up the impl map. The resolver
collects all `HirItem::Impl` blocks, including `impl Drop for T`.

### 2.3 Drop elaboration — ALREADY WORKS (no-op)

`elaborate_drops` (Stage 15.44) uses `ty_needs_drop` (Stage 15.43)
which calls `resolver.is_drop_builtin`. If a type implements `Drop`,
`ty_needs_drop` returns `true`, and `elaborate_drops` inserts `Drop`
terminators.

### 2.4 Drop glue codegen — CRASHES

The `TerminatorKind::Drop` codegen (Stage 15.45) calls
`drop_adt_<DefId>` — but this function is never emitted. The drop
glue function emission was identified as remaining work in the Stage
15.47 gate review but was not implemented.

**Result**: When `elaborate_drops` inserts a `Drop` terminator and
codegen tries to call `drop_adt_<N>`, the LLVM module references an
undefined function, causing a crash.

## 3. What Needs to Be Done

The remaining work is **Stage 15.57: Drop glue function emission**:

1. For each type that implements `Drop` (or has fields that need drop),
   emit a `drop_adt_<DefId>` function that:
   - Calls the user's `Drop::drop` method (if `impl Drop` exists).
   - Then recursively drops each field (if the field type needs drop).

2. Register the `drop_adt_<DefId>` function in the codegen so it's
   emitted as part of the LLVM module.

This is the work that was planned for Stage 15.57 but is now the
**primary remaining work** for Task 13.

## 4. Revised Implementation Plan

| Stage | Description | Status |
|-------|-------------|--------|
| 15.55 | Phase 3 design alignment | ✅ DONE |
| **15.56** | **Parser investigation** (this stage — parser already works) | **✅ DONE** |
| 15.57 | **Drop glue function emission** (the key remaining work) | ⏳ NEXT |
| 15.58 | Conformance tests with `impl Drop` patterns | ⏳ PLANNED |
| 15.59 | Gate review | ⏳ PLANNED |

## 5. Verification

- No code changes — investigation-only stage.
- All existing tests pass (zero regression).
- The crash only occurs when a program has `impl Drop` (which no
  existing test has — the parser support was always there, but no
  test exercised it with codegen).
