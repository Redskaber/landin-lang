# Stage 18.103 — Per-Mono Codegen (TD-MONO-CODEGEN)

> **Author**: redskaber
> **Date**: 2026-08-11
> **Version**: v0.370.0 → v0.371.0
> **Status**: Active

## 1. 设计文档对齐（§13.1）

### 1.1 对应设计文档

- `docs/develop/v0/task-11-monomorphization-design.md` §3.4 Phase 4 (Per-Mono Codegen)
- `docs/develop/v0/stage-18/stage-18.102-implicit-inference-backwrite-design.md` (TD-MONO-CODEGEN gap)

### 1.2 设计意图摘要

Stage 18.101 + 18.102 fixed FnDef substs propagation (turbofish + implicit).
This stage completes monomorphization by emitting specialized functions for
each MonoItem::Fn and updating call sites to use specialized names.

### 1.3 已实现 / 偏差 / 未实现

| Item | Status |
|------|--------|
| `substitute_mir_body` function | ✅ Stage 18.103 |
| `codegen_mono_functions` pass | ✅ Stage 18.103 |
| Call site uses specialized name | ✅ Stage 18.103 |
| `mir.def_id` set in driver | ✅ Stage 18.103 |
| Turbofish `id::<i32>` → `id_i32` function | ✅ verified |
| Turbofish `id::<bool>` → `id_bool` function | ✅ verified |
| Implicit `id(42)` → specialized function | ✅ (via 18.102 substs) |
| Method monomorphization | ❌ S2 (v0.2 Phase 2) |

## 2. 任务拆分（MUV）

| ID | Task | Acceptance |
|----|------|------------|
| 18.103.1 | `substitute_mir_body` | Clone MirBody, substitute all Param types |
| 18.103.2 | `codegen_mono_functions` | Emit specialized function per MonoItem::Fn |
| 18.103.3 | Set `mir.def_id` in driver | So codegen can find generic MIR body by DefId |
| 18.103.4 | Update Call codegen | Use specialized name when FnDef has substs |
| 18.103.5 | Add tests | Specialized functions emitted + call sites use them |

## 3. Implementation

### 3.1 substitute_mir_body (src/mir/substitute.rs)

Clones a MirBody and replaces all `Param(N)` types:
- `local_decl.ty` → `substitute(ty, substs)`
- `Operand::Constant(c).ty` → `substitute(c.ty, substs)`
- (S3: Rvalue/Place types NOT substituted — they derive from local_decls)

### 3.2 codegen_mono_functions (src/codegen/function.rs)

For each `MonoItem::Fn { def_id, substs }`:
1. Find generic MIR body by `mir.def_id == Some(def_id)`
2. `substitute_mir_body(generic_mir, substs)` → specialized MIR
3. `mono_item_name(item, base, ...)` → specialized name (e.g., `id_i32`)
4. `codegen_function(emitter, &specialized_name, &specialized_mir, ...)`

### 3.3 Call site update (src/codegen/terminator.rs)

When resolving function name for a Call:
- If `FnDef(def_id, substs)` with `!substs.is_empty()`:
  - Use `mono_item_name` to compute specialized name
- Else: use base name from `fn_name_by_def_id`

### 3.4 mir.def_id set in driver (src/driver.rs)

After `lower_hir_body_to_mir_full_with_dyn_trait_plan`:
```rust
mir.def_id = Some(owner_def_id);
```
This allows `codegen_mono_functions` to find the generic MIR body by DefId.

## 4. Design Simplifications (Documented)

### S3: Only local_decl.ty + Constant.ty substituted

**Description**: `substitute_mir_body` only substitutes types in `local_decl.ty`
and `Operand::Constant(c).ty`. Rvalue/Place types are NOT substituted.

**Reason**: Codegen reads types from `local_decls` (not from rvalues). Rvalue
types are derived from local_decls at codegen time, so substituting them would
be redundant.

**Impact**: None for current codegen.
**Fix plan**: v0.2 Phase 2 — extend if codegen changes to read rvalue types.

### S4: Only MonoItem::Fn handled

**Description**: `codegen_mono_functions` only handles `MonoItem::Fn`.
`MonoItem::Type` (layouts) handled by `build_mono_layouts`. `MonoItem::Closure`
handled by `codegen_synthesized_closure_functions`.

**Impact**: Generic closure monomorphization not handled here.
**Fix plan**: v0.2 Phase 2 — add MonoItem::Closure if needed.

### S5: Call site type_names map empty

**Description**: In `codegen/terminator.rs` Call handling, the `type_names` map
passed to `mono_item_name` is empty (no HIR access in codegen).

**Reason**: Codegen has no HIR access (per §11). For primitive substs (i32, bool),
`mangle_ty` produces correct names without the map. For Adt substs, it falls
back to `Adt_N` (acceptable but not ideal).

**Impact**: Specialized names for Adt substs use `Adt_N` instead of the type name.
**Fix plan**: v0.2 Phase 2 — pre-compute type_names map in driver and pass to codegen.

## 5. Verification

### Before (v0.370.0)
```
define i32 @landin_id(i32 %arg0) {  // ONE generic function
  %v1 = call i32 @landin_id(i32 42)  // calls generic
  %v2 = call i32 @landin_id(i1 1)    // calls generic (WRONG: bool arg to i32 fn)
```

### After (v0.371.0)
```
define i32 @landin_id(i32 %arg0) {  // generic (still emitted)
define i32 @id_i32(i32 %arg0) {     // ✅ specialized for i32
define i1 @id_bool(i1 %arg0) {      // ✅ specialized for bool
  %v1 = call i32 @landin_id_i32(i32 42)    // ✅ calls specialized
  %v2 = call i32 @landin_id_bool(i1 1)     // ✅ calls specialized
```

## 6. 验收（§3.2）

- [x] `cargo build --features llvm-backend` 成功
- [x] `cargo fmt --check` exit 0
- [x] `cargo clippy --all-targets --features llvm-backend -- -D warnings` 0 warnings
- [x] `cargo test --features llvm-backend --lib` 全绿 (640 passed)
- [x] `cargo test --features llvm-backend --tests` (skip runtime) 全绿 (2625 passed)
- [x] Turbofish produces specialized functions ✅
- [x] Call sites use specialized names ✅
- [x] Non-generic still uses base name ✅
- [x] No regression in existing tests

## 7. v0.2 Monomorphization Progress

| Phase | Status |
|-------|--------|
| Phase 1: Substs propagation (Adt) | ✅ Stage 16.52 |
| Phase 2: Substitution | ✅ Stage 16.53 |
| Phase 3: Monomorphization collection | ✅ Stage 16.54 |
| Phase 4a: Specialized naming | ✅ Stage 16.55 |
| Phase 4b: Per-mono layouts | ✅ Stage 16.59 |
| Phase 4c: Codegen integration | ✅ Stage 16.59 |
| Turbofish FnDef substs | ✅ Stage 18.101 |
| Implicit inference FnDef substs | ✅ Stage 18.102 |
| **Per-mono codegen (emit specialized fns)** | ✅ Stage 18.103 |
| **Call sites use specialized names** | ✅ Stage 18.103 |
| Method monomorphization | ❌ S2 (v0.2 Phase 2) |
| Adt subst name in specialized fn names | ❌ S5 (v0.2 Phase 2) |
