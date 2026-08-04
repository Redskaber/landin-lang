# Stage 16.53 — Task 11 Phase 2: Type Substitution

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.238.0 → v0.239.0
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.53 implements Task 11 Phase 2 — the `substitute(ty, substs)` function
and its integration into field type resolution. Phase 1 (Stages 16.50-16.52)
propagated substs into MIR types; Phase 2 makes those substs actually useful
by replacing `TyKind::Param` placeholders with concrete types.

**What was implemented**:

1. **`src/mir/substitute.rs`** — new module with:
   - `substitute(ty, substs) -> Ty` — pure function, replaces `Param(idx)` with
     `substs[idx]`, recursively substitutes inner types
   - `substitute_substs(inner_substs, outer_substs) -> Vec<Ty>` — substitutes
     a substs slice (for nested generics like `Vec<Vec<i32>>`)
   - `substitute_const(c, substs) -> Const` — substitutes const's type
   - `substitute_sig(sig, substs) -> Sig` — substitutes fn signature
   - 29 unit tests covering all TyKind variants

2. **`lower_hir_ty_to_mir_ty_with_generics`** — new function in
   `src/mir/lower/mod.rs` that resolves generic type parameters (e.g., `T` in
   `struct Box<T> { val: T }`) to `TyKind::Param(ParamTy { index, name })`
   instead of `TyKind::Error`. This is the key step that makes `substitute`
   useful.

3. **`resolve_adt_field_tys_with_substs`** — new function in
   `src/mir/lower/field_resolution.rs` that:
   - Gets the ADT's generic params via `generics_of`
   - Lowers each field type with generic param resolution
   - Applies `substitute` with the ADT's substs
   - Returns the substituted (concrete) field types

4. **`resolve_field_type`** updated to use substitution when the receiver's
   type has non-empty substs. `find_receiver_substs` helper extracts substs
   from the receiver's MIR type.

5. **`is_mir_ty_copy_conservative`** + `ty_is_copy` + `ty_is_copy_with_resolver`
   updated to treat `Param` as Copy (same as `Infer`/`Error`). This avoids
   spurious "use of moved value" errors during type inference — the concrete
   type behind a `Param` is only known after monomorphization.

6. **`lower_ast_ty_to_mir_ty`** updated to produce `Error` (instead of
   `Adt(DefId(0), [])`) for unresolved paths in generic args. The dummy Adt
   was causing spurious move errors when used as a subst.

7. **18 integration tests** in
   `tests/v0/stage16/plan/stage16_53_substitute_tests.rs` covering:
   - substitute function unit tests (3)
   - Generic struct field access (4)
   - Generic enum compilation (2)
   - No regressions on non-generic code (3)
   - MIR inspection (2)
   - Complex generic patterns (4)

**Key result**: `let b: Box<i32> = Box { val: 42 }; b.val` now compiles
end-to-end. The field `val: T` is lowered as `Param(ParamTy { index: 0 })`,
substituted with `i32` (from `b`'s substs), producing the concrete type
`i32` for `b.val`.

**Test results**: 7973 tests passing (279 lib + 2470 integration + 5224
conformance), 0 failures, 0 warnings. +47 new tests (29 unit + 18 integration).

## 2. Design Decisions

### 2.1 Pure Function + Integration (通解 > 特解)

The `substitute` function is pure — no HIR access, no side effects, no
resolver. It operates solely on `Ty` values. This follows §1.0 原則 6
"通用 > 特例" — one function for all type kinds, dispatched via match.

The integration is separate: `resolve_adt_field_tys_with_substs` and
`resolve_field_type` call `substitute` after lowering field types with
generic param resolution. This separation follows §16 (interface isolation)
— the pure function doesn't know about HIR; the integration function
doesn't know about the substitution algorithm.

### 2.2 Param as Copy (显式 > 隐式)

During type inference and borrowck, `Param(X)` represents a generic type
parameter whose concrete type is only known after monomorphization (Phase 4).
We can't know if `X` is Copy, so we conservatively assume Copy to avoid
spurious "use of moved value" errors (e.g., `self.x.f()` where `f` takes
`&self` and `self.x: X`).

This is the same treatment as `Infer` and `Error` — all three represent
"unknown type" states. The actual Copy-ness will be checked after
monomorphization when the concrete type is substituted in.

Per §1.0 原則 5 "报错 > 静默": false negative (missed move error) is
preferred over false positive (spurious move error) during inference.

### 2.3 Error for Unresolved Paths (报错 > 静默)

`lower_ast_ty_to_mir_ty` produces `Error` for unresolved paths in generic
args (e.g., `X` in `S<X>`). Previously, it produced `Adt(DefId(0), [])` —
a dummy Adt that caused two problems:
1. `Adt` is not Copy → spurious "use of moved value" errors
2. `Adt(DefId(0))` is a meaningless type that pollutes typeck

`Error` is the existing convention for "unresolved type" throughout the
compiler. It's Copy and doesn't trigger the `bind_ty_var` panic path.

### 2.4 Generic Param Resolution (通用 > 特例)

`lower_hir_ty_to_mir_ty_with_generics` resolves type parameters by checking
if a path's single segment name matches one of the `generic_params`. This
is a simple name-based match — no scope tracking, no resolver. It works
because generic params are always single-segment paths (e.g., `T`, not
`mod::T`).

Per §1.0 原則 6 "通用 > 特例": one function for all generic type lowering,
no special cases for specific kinds of generics.

## 3. Changes

### 3.1 New Module: `src/mir/substitute.rs`

```rust
pub fn substitute(ty: &Ty, substs: &[Ty]) -> Ty
pub fn substitute_substs(inner_substs: &SubstsRef, outer_substs: &[Ty]) -> Vec<Ty>
fn substitute_const(c: &Const, substs: &[Ty]) -> Const
fn substitute_sig(sig: &Sig, substs: &[Ty]) -> Sig
```

### 3.2 New Function: `lower_hir_ty_to_mir_ty_with_generics`

In `src/mir/lower/mod.rs`:
```rust
pub(crate) fn lower_hir_ty_to_mir_ty_with_generics(
    ty: &HirTy,
    generic_params: &[ParamTy],
) -> Ty
```

Resolves single-segment paths with `Res::Err`/`Res::Unknown` to `TyKind::Param`
if the name matches a generic param. Recursively handles Tuple, Ref, Ptr,
Slice, Array. Delegates to `lower_hir_ty_to_mir_ty_with_regions` for other kinds.

### 3.3 New Function: `resolve_adt_field_tys_with_substs`

In `src/mir/lower/field_resolution.rs`:
```rust
pub(crate) fn resolve_adt_field_tys_with_substs(
    cx: &MirLowerCtxt,
    def_id: DefId,
    substs: &SubstsRef,
) -> Vec<Ty>
```

Gets generic params, lowers fields with generics, applies substitution.
Falls back to plain `resolve_adt_field_tys` for non-generic ADTs (empty
substs or no generic params).

### 3.4 Updated: `resolve_field_type`

Now extracts substs from the receiver's type (`find_receiver_substs`) and
applies substitution when substs are non-empty.

### 3.5 Updated: `is_mir_ty_copy_conservative` + `ty_is_copy` + `ty_is_copy_with_resolver`

`Param(_)` moved from "non-Copy" to "Copy" (same as `Infer`/`Error`/`Foreign`).

### 3.6 Updated: `lower_ast_ty_to_mir_ty`

`ATy::Path` arm now produces `Error` instead of `Adt(DefId(0), [])`.

### 3.7 Updated: AggregateKind::Adt sites in `expr_operand.rs`

Sites 2 and 3 now use `resolve_adt_field_tys_with_substs` when substs are
non-empty.

## 4. API (§23 Compliant)

| Function | Pattern | Location |
|----------|---------|----------|
| `substitute` | `<verb>` — pure function | `src/mir/substitute.rs` |
| `substitute_substs` | `<verb>_<noun>` — pure function | `src/mir/substitute.rs` |
| `substitute_const` | `<verb>_<noun>` — pure function | `src/mir/substitute.rs` |
| `substitute_sig` | `<verb>_<noun>` — pure function | `src/mir/substitute.rs` |
| `lower_hir_ty_to_mir_ty_with_generics` | `<verb>_<noun>_<noun>_<prep>_<noun>` | `src/mir/lower/mod.rs` |
| `lower_hir_ty_to_mir_ty_with_generics_and_regions` | `<verb>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` | `src/mir/lower/mod.rs` |
| `resolve_adt_field_tys_with_substs` | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` | `src/mir/lower/field_resolution.rs` |
| `find_receiver_substs` | `<verb>_<noun>_<noun>` | `src/mir/lower/field_resolution.rs` |

## 5. Test Plan

29 unit tests in `src/mir/substitute.rs` + 18 integration tests in
`tests/v0/stage16/plan/stage16_53_substitute_tests.rs`.

### Unit Tests (29)

| Category | Tests | Description |
|----------|-------|-------------|
| Leaf types | 5 | Bool, Int, Str, Never, Error — no substitution |
| Param | 4 | Replacement, second index, out of bounds, empty substs |
| Ref | 1 | Substitute inner |
| RawPtr | 1 | Substitute inner |
| Array | 1 | Substitute inner + const |
| Slice | 1 | Substitute inner |
| Tuple | 1 | Substitute each element |
| Adt | 3 | Single subst, multiple substs, empty substs |
| FnDef | 1 | Substitute inner substs |
| Closure | 1 | Substitute inner substs |
| FnPtr | 1 | Substitute inputs + output |
| Infer | 1 | Not substituted (resolved by typeck) |
| Nested | 3 | Vec<Vec<T>>, &Box<T>, (T, T, U) |
| substitute_substs | 3 | Basic, empty, no params |
| Idempotency | 2 | Empty substs on leaf and Adt |

### Integration Tests (18)

| Category | Tests | Description |
|----------|-------|-------------|
| substitute function | 3 | Param replacement, leaf noop, substs slice |
| Generic struct field access | 4 | Single param, two params, method body, trait impl |
| Generic enum | 2 | Match, unit variant |
| No regressions | 3 | Non-generic struct, struct method, enum |
| MIR inspection | 2 | Local has substs, field access produces concrete type |
| Complex patterns | 4 | Nested generic, tuple field, ref field, multiple structs |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 279/279 PASS (+29 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2470/2470 PASS (+18 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7973 tests passing, 0 failures, 0 warnings.**

## 7. Version Policy

v0.238.0 → v0.239.0 (minor bump — new module + new functions + typeck
behavior change: Param is now treated as Copy during inference. Field types
for generic structs are now substituted correctly.)

## 8. Next Steps (Task 11 Roadmap)

| Phase | Status | Stage | Description |
|-------|--------|-------|-------------|
| 1a | ✅ | 16.50 | `generics_of` query |
| 1b | ✅ | 16.51 | Substs propagation into `TyKind::Adt` |
| 1c | ✅ | 16.52 | Substs propagation into `AggregateKind::Adt` |
| 2 | ✅ | 16.53 | `substitute(ty, substs)` function + integration |
| 3 | 🔧 Next | — | Monomorphization collection (`collect_mono_items`) |
| 4 | 🔧 Planned | — | Per-mono codegen |

## 9. References

- Stage 16.52 design: `docs/develop/v0/stage-16/stage-16.52-aggregate-substs-propagation.md`
- Stage 16.51 design: `docs/develop/v0/stage-16/stage-16.51-substs-propagation.md`
- Stage 16.50 design: `docs/develop/v0/stage-16/stage-16.50-generics-of-query.md`
- Task 11 design: `docs/develop/v0/task-11-monomorphization-design.md`
- Type system data flow: `docs/graph/type-system/data-flow.md`
- Stage Committee process: `docs/stage-committee-process.md` §13.4 + §23
