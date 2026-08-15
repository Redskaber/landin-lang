# Stage 18.104 — Monomorphization Return Type Fix Investigation (S6)

> **Author**: redskaber
> **Date**: 2026-08-11
> **Version**: v0.371.0 → v0.372.0
> **Status**: Active

## 1. 设计文档对齐（§13.1）

### 1.1 对应设计文档

- `docs/develop/v0/stage-18/stage-18.103-per-mono-codegen-design.md` (S3/S4/S5 simplifications)
- `docs/develop/v0/task-11-monomorphization-design.md` §3.4 Phase 4

### 1.2 设计意图摘要

Stage 18.103 completed per-mono codegen, but testing revealed a new issue (S6):
generic function return types with nested Param (e.g., `Box<T>` in `fn make_box<T>(x: T) -> Box<T>`)
produce `Adt(DefId, [Error])` instead of `Adt(DefId, [Param(0)])` in fn_sig_table.
This causes specialized functions to have wrong return types.

## 2. Root Cause Analysis (S6)

### 2.1 Symptom

```
fn make_box<T>(x: T) -> Box<T> { Box { val: x } }
```

After `compile()`:
- `fn_sigs[make_box].output = Adt(Box_def_id, [Error])` ❌ (should be `[Param(0)]`)
- Specialized `make_box_bool` function returns `{ i32 }` ❌ (should return `{ i1 }`)

### 2.2 Root Cause

`lower_hir_ty_to_mir_ty(Box<T>)` processes the `Box` path → `Res::Def(Box_def_id)` → calls
`lower_path_generic_args(path, ...)` to extract substs from `<T>`.

`lower_path_generic_args` reads `path.segments.last().args` = `<T>` (AST `GenericArg::Type(Ty)`),
then calls `lower_ast_ty_to_mir_ty(T, hir)`.

`lower_ast_ty_to_mir_ty(T, hir)`:
- `T` is `ATy::Path("T")`
- `lookup_type_def_id_by_name(hir, "T")` → `None` (T is a type parameter, not a struct/enum)
- Falls back to `TyKind::Error` ❌

### 2.3 Why lower_ast_ty_to_mir_ty doesn't handle type parameters

`lower_ast_ty_to_mir_ty` works at the AST level — it has no resolver/generics context.
It can only look up struct/enum names by scanning HIR owners. Type parameters (`T`) are
not HIR owners; they're `HirGenericParam` inside the function/struct definition.

### 2.4 Fix Direction

**Option A (correct but large)**: Pass `generic_params: &[ParamTy]` to `lower_ast_ty_to_mir_ty`
so it can check if a path name matches a type parameter. This requires updating all call sites.

**Option B (simpler)**: In `lower_path_generic_args`, when lowering a generic arg that is a
bare type parameter (single-segment path, name matches a known generic param), produce
`Param(N)` directly instead of calling `lower_ast_ty_to_mir_ty`.

**Option C (workaround)**: After fn_sig_table is built, run a post-pass that replaces
`Error` substs in generic function sigs with `Param(N)` based on the function's generics.

## 3. Design Simplification (S6)

### S6: Generic return type Param not propagated through lower_ast_ty_to_mir_ty

**Description**: When a generic function's return type contains a generic type parameter
(e.g., `Box<T>` in `fn f<T>() -> Box<T>`), the `T` in the return type is lowered to
`TyKind::Error` instead of `TyKind::Param(0)` because `lower_ast_ty_to_mir_ty` cannot
resolve bare type parameter names.

**Reason**: `lower_ast_ty_to_mir_ty` works at AST level with no generics context. It only
resolves struct/enum names by scanning HIR owners. Type parameters are not HIR owners.

**Impact**: Specialized functions for generic functions with Param-containing return types
have wrong return types. E.g., `make_box<bool>` returns `{ i32 }` instead of `{ i1 }`.

**Scope**: Only affects generic functions whose return type contains a type parameter
nested inside an Adt (e.g., `Box<T>`, `Vec<T>`, `Pair<T, U>`). Functions with direct
Param return (e.g., `fn id<T>(x: T) -> T`) work correctly because the return type IS
Param (not nested inside Adt).

**Fix plan**: v0.2 Phase 2 — implement Option B or C above. Option B is cleaner (fix at
the source) but requires passing generics context to `lower_path_generic_args`.

## 4. Current Stage Action

This stage documents S6 and creates a design doc + plan. The actual fix is deferred to
v0.2 Phase 2 because:
1. It requires touching `lower_ast_ty_to_mir_ty` (used in many places)
2. The impact is limited to nested-Param return types
3. The current monomorphization core (turbofish + implicit + per-mono codegen) works
   for the common case (direct Param return like `fn id<T>(x: T) -> T`)

## 5. 验收

- [x] S6 root cause documented
- [x] Design doc created
- [x] Fix plan defined (Option B for v0.2 Phase 2)
- [x] No code changes (investigation only)
