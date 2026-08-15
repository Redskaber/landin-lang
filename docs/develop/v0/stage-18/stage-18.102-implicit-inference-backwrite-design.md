# Stage 18.102 — Implicit Generic Inference Back-Write (TD-MONO-INFER)

> **Author**: redskaber
> **Date**: 2026-08-11
> **Version**: v0.369.0 → v0.370.0
> **Status**: Active

## 1. 设计文档对齐（§13.1）

### 1.1 对应设计文档

- `docs/develop/v0/task-11-monomorphization-design.md` §3.4 Phase 4 (Per-Mono Codegen)
- `docs/develop/v0/stage-18/stage-18.101-turbofish-monomorphization-design.md` (TD-MONO-INFER gap)
- `docs/develop/v0/v0.1-capability-boundaries.md` v0.2 Roadmap P0 Monomorphization

### 1.2 设计意图摘要

Stage 18.101 fixed turbofish monomorphization (`id::<i32>(42)`), but implicit
generic calls (`id(42)` without turbofish) still produced empty FnDef substs
because MIR lowering happens before type inference. This stage implements a
writeback-style pass that infers substs from arg/return types after typeck.

### 1.3 已实现 / 偏差 / 未实现

| Item | Status |
|------|--------|
| `writeback_fndef_substs` pass | ✅ Stage 18.102 |
| `generics_map` pre-computed from HIR | ✅ Stage 18.102 |
| `collect_param_bindings` helper | ✅ Stage 18.102 |
| Implicit `id(42)` → MonoItem::Fn{i32} | ✅ verified |
| Mixed turbofish + implicit | ✅ verified |
| Per-mono codegen (emit specialized fns) | ❌ TD-MONO-CODEGEN (v0.2) |

## 2. 任务拆分（MUV）

| ID | Task | Acceptance |
|----|------|------------|
| 18.102.1 | Implement `writeback_fndef_substs` | Walks Call terminators, matches arg types with Param(N), writes back substs |
| 18.102.2 | Pre-compute `generics_map` from HIR | DefId → Vec<ParamTy> for all generic items |
| 18.102.3 | Wire into driver | Called after `writeback_closures`, before MIR opt |
| 18.102.4 | Add implicit inference tests | `id(42)` + `id(true)` → 2 MonoItems |
| 18.102.5 | Document S1/S2 simplifications | Design doc + code comments |

## 3. Algorithm

### 3.1 Core Algorithm

For each `Call { func: Copy(local), args, destination, .. }` terminator:

1. Read `local_decls[local].ty` — if `FnDef(def_id, [])` (empty substs):
2. Look up `generics_map[def_id]` — skip if not generic
3. Look up `fn_sigs[def_id]` to get the sig (inputs contain `Param(N)`)
4. For each `(arg, input_ty)` pair:
   - Resolve the arg's type from local_decls
   - If `input_ty` is `Param(N)`, record `bindings[N] = arg_ty`
5. Also check the return type: if `sig.output` is `Param(N)`, use the
   destination local's type as `bindings[N]`
6. Build the substs vector from bindings (ordered by param index)
7. Write back `FnDef(def_id, substs)` to `local_decls[local].ty`

### 3.2 Example

```landin
fn id<T>(x: T) -> T { x }
fn main() {
    let a: i32 = id(42);  // arg=42:i32, ret=i32
}
```

After MIR lowering: `local_1: FnDef(id, [])` (empty substs)
After typeck: `local_2: i32` (arg), `local_3: i32` (dest)

`writeback_fndef_substs`:
- sig.inputs = `[Param(0)]`, arg type = `i32` → bindings[0] = i32
- sig.output = `Param(0)`, dest type = `i32` → bindings[0] = i32 (redundant)
- substs = `[i32]`
- Write back: `local_1: FnDef(id, [i32])`

Result: `collect_mono_items` finds `MonoItem::Fn { id, [i32] }` ✅

## 4. Design Simplifications (Documented per user request)

### S1: Only top-level Param types matched

**Description**: `collect_param_bindings` only matches `Param(N)` at the top
level of a sig input/output type. Nested Params (e.g., `fn foo<T>(x: Vec<T>)`
where the input is `Adt(Vec, [Param(0)])`) are NOT extracted.

**Reason**: Full nested param extraction requires recursive type matching
(walk Adt substs, Ref inner, Tuple elements, etc.). This is more complex
and was deferred to keep the initial fix focused.

**Impact**: Generic functions with nested param types in their sig (e.g.,
`fn wrap<T>(x: Vec<T>)`) won't get substs for `T`. The FnDef remains empty,
and no MonoItem is collected.

**Fix plan**: v0.2 Phase 2 — extend `collect_param_bindings` to recurse into
nested types. The recursion logic already exists in `collect_from_ty` (in
`mir/monomorphize/item.rs`) and can be adapted.

### S2: Only Copy/Move func operands handled

**Description**: Only `Operand::Copy(Place::local(id))` and `Operand::Move(...)`
func operands are handled. `Operand::Constant(Const { ty: FnDef(...), .. })`
func operands (used in some method call paths) are NOT handled.

**Reason**: The constant case requires mutating the Const's type (which is
inside an Operand inside a Terminator), more complex than the local_decl
case. Method calls use the Constant path, so this affects method monomorphization.

**Impact**: Generic method calls (`x.method::<i32>()` or `x.method(42)`) won't
get substs written back via this pass. Method monomorphization is deferred.

**Fix plan**: v0.2 Phase 2 — handle Constant func operands by walking the
Terminator's func operand and mutating the Const's type in place.

## 5. API Naming Compliance (§10)

- ✅ `writeback_fndef_substs` follows `<verb>_<noun>_<noun>` pattern (§23)
- ✅ `collect_param_bindings` follows `<verb>_<noun>_<noun>` pattern
- ✅ `find_generics` (existing) follows `<verb>_<noun>` pattern
- ✅ No glob re-exports — explicit `pub use writeback_fndef_substs`
- ✅ Single source of truth — one writeback function for all generic calls

## 6. Interface Isolation (§11)

- ✅ `writeback_fndef_substs` takes `&mut MirBody` + `&FnSigTable` + `&generics_map`
- ✅ No HIR access during writeback (generics_map pre-computed from HIR)
- ✅ Driver is orchestrator — calls writeback in correct order
- ✅ Pure MIR-to-MIR transform (like `writeback_type_propagation` + `writeback_closures`)

## 7. 验收（§3.2）

- [x] `cargo build --features llvm-backend` 成功
- [x] `cargo fmt --check` exit 0
- [x] `cargo clippy --all-targets --features llvm-backend -- -D warnings` 0 warnings
- [x] `cargo test --features llvm-backend --lib` 全绿 (640 passed)
- [x] `cargo test --features llvm-backend --tests` (skip runtime) 全绿 (2622 passed)
- [x] Implicit `id(42)` + `id(true)` → 2 MonoItems ✅
- [x] Non-generic `add(1,2)` → 0 MonoItems ✅
- [x] Mixed turbofish + implicit → 2 MonoItems ✅
- [x] No regression in existing tests

## 8. v0.2 Monomorphization Progress

| Phase | Status |
|-------|--------|
| Phase 1: Substs propagation (Adt) | ✅ Stage 16.52 |
| Phase 2: Substitution | ✅ Stage 16.53 |
| Phase 3: Monomorphization collection | ✅ Stage 16.54 |
| Phase 4a: Specialized naming | ✅ Stage 16.55 |
| Phase 4b: Per-mono layouts | ✅ Stage 16.59 |
| Phase 4c: Codegen integration | ✅ Stage 16.59 |
| Turbofish FnDef substs | ✅ Stage 18.101 |
| **Implicit inference FnDef substs (TD-MONO-INFER)** | ✅ Stage 18.102 |
| Per-mono codegen (emit specialized fns) | ❌ v0.2 (TD-MONO-CODEGEN) |
