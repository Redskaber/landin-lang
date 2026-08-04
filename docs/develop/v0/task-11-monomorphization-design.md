# Task 11 Design Document — Monomorphization (Generic Support)

> **Author**: redskaber
> **Date**: 2026-08-04 (Stage 16.49)
> **Version**: v0.243.0 (Stage 16.57)
> **Process**: stage-committee-process.md v3.24 §13.4 (stage-start design alignment)
> **Status**: Phase 1-3 + 4a + 4b-pre + 4b complete, Phase 4c planned

## 1. Executive Summary

Task 11 implements monomorphization — the process of generating specialized
code for each concrete instantiation of a generic type or function. This is
the foundation for `Vec<T>`, `HashMap<K,V>`, and other generic types.

**Key finding**: Generic syntax parsing is fully implemented (parser → AST →
HIR → MIR type slots). The MIR lowerer discards parsed generic args — always
emitting empty `SubstsRef`. The plumbing exists; the data flow is severed at
the HIR→MIR boundary.

## 2. Current State

### 2.1 What Works (Plumbing)

| Layer | Status | Details |
|-------|--------|---------|
| Parser | ✅ | `parse_generics()`, `try_parse_generic_args()`, turbofish |
| AST | ✅ | `Generics`, `GenericParam`, `GenericArg`, `PathSegment.args` |
| HIR | ✅ | `HirGenerics`, `HirPathSegment.args` preserved |
| MIR Type Slots | ✅ | `SubstsRef = Rc<[Ty]>`, `TyKind::Adt(DefId, SubstsRef)`, `TyKind::Param(ParamTy)` |

### 2.2 What's Broken (Data Flow)

| Gap | Location | Description |
|-----|----------|-------------|
| Substs propagation | `mir/lower/mod.rs:1739` | `path.args` never inspected, always `Vec::new().into()` |
| Aggregate construction | `mir/lower/expr_operand.rs` | 16 sites using empty substs |
| Generics query | — | No `generics_of(DefId)` query exists |
| Substitution | — | No `substitute(ty, substs)` function |
| Monomorphization | — | No collection pass |
| Per-mono codegen | `codegen/mir_translation.rs` | Layouts keyed by DefId only, no specialization |

## 3. Design

### 3.1 Phase 1: Substs Propagation ✅ COMPLETE (Stages 16.50-16.52)

**Goal**: `let x: Vec<i32>` produces `Adt(Vec_def_id, [i32])` in MIR,
and `Vec { ... }` aggregate construction carries the same substs.

**Step 1a (Stage 16.50)** ✅: Create `generics_of` query
```rust
// In src/hir/generics.rs
pub fn generics_of(hir: &HirCrate, def_id: DefId) -> Vec<ParamTy>
```
Walks HIR items, collects type parameters into a `DefId → Vec<ParamTy>` map.

**Step 1b (Stage 16.51)** ✅: Propagate substs into `TyKind::Adt`
```rust
// In src/mir/lower/mod.rs
Res::Def(def_id, _) => {
    let substs = lower_path_generic_args(path, region_counter);
    Ty::new(TyKind::Adt(def_id, substs), span)
}
```

**Step 1c (Stage 16.52)** ✅: Propagate substs into `AggregateKind::Adt`
at all 5 construction sites in `src/mir/lower/expr_operand.rs`:
- Enum unit variant path (`Color::Red`)
- ADT ctor call (`Pair(1, 2)`)
- Struct literal (`Pair { a: 1, b: 2 }`)
- Enum struct variant (`Shape::Circle { r: 1.0 }`)
- Fall-through ADT ctor path

Plus: reworked `typeck/unify.rs` Adt unification — replaced the temporary
Stage 16.51 relaxation with the principled "empty substs = unknown" rule.

### 3.2 Phase 2: Substitution ✅ COMPLETE (Stage 16.53)

**Goal**: Given `struct Vec<T> { data: [T; N], len: usize }` and substs `[i32]`,
produce field type `[i32; N]` for the `data` field.

**Step 2a** ✅: Create `substitute(ty, substs)` function
```rust
// In src/mir/substitute.rs
pub fn substitute(ty: &Ty, substs: &[Ty]) -> Ty {
    match &ty.kind {
        TyKind::Param(ParamTy { index, .. }) => substs[*index as usize].clone(),
        TyKind::Adt(def_id, inner_substs) => {
            let substituted = inner_substs.iter().map(|t| substitute(t, substs));
            Ty::new(TyKind::Adt(*def_id, substituted.collect::<Vec<_>>().into()), span)
        }
        // ... recursively handle Ref, Tuple, Array, etc.
    }
}
```

**Step 2b** ✅: Create `lower_hir_ty_to_mir_ty_with_generics` — resolves type
parameters (e.g., `T`) to `TyKind::Param(ParamTy { index, name })` instead of
`TyKind::Error`.

**Step 2c** ✅: Integrate into `resolve_adt_field_tys_with_substs` — lowers
field types with generic param resolution, then applies substitution.

**Step 2d** ✅: Update `resolve_field_type` to use substitution when receiver
has substs. Added `find_receiver_substs` helper.

**Step 2e** ✅: Update `is_mir_ty_copy_conservative` + `ty_is_copy` +
`ty_is_copy_with_resolver` to treat `Param` as Copy (same as `Infer`/`Error`).

**Step 2f** ✅: Update `lower_ast_ty_to_mir_ty` to produce `Error` (not
`Adt(DefId(0), [])`) for unresolved paths in generic args.

### 3.3 Phase 3: Monomorphization Collection ✅ COMPLETE (Stage 16.54)

**Goal**: Walk all MIR bodies, collect `MonoItem { def_id, substs }` pairs,
dedup.

```rust
// In src/mir/monomorphize.rs
pub enum MonoItem {
    Type { def_id: DefId, substs: SubstsRef },
    Fn { def_id: DefId, substs: SubstsRef },
    Closure { def_id: DefId, substs: SubstsRef },
}

pub fn collect_mono_items(mirs: &[MirBody]) -> Vec<MonoItem> {
    // Walk all MIR bodies
    // For each Adt(def_id, substs) where !substs.is_empty(): collect
    // For each FnDef(def_id, substs) where !substs.is_empty(): collect
    // For each Closure(def_id, substs) where !substs.is_empty(): collect
    // Recursively substitute into generic function bodies
    // Dedup by (def_id, substs) via HashSet
}
```

**Step 3a** ✅: Created `MonoItem` enum with `Type`, `Fn`, `Closure` variants.
Each carries `def_id` and `substs`. Derives `Eq + Hash` for HashSet dedup.

**Step 3b** ✅: Created `collect_mono_items(mirs) -> Vec<MonoItem>` — walks
all MIR bodies, collects MonoItems from local_decls, statements, terminators.

**Step 3c** ✅: Created `collect_from_ty(ty, collected)` — recursive type
walker that extracts MonoItems from `Adt`/`FnDef`/`Closure` with non-empty
substs. Recursively walks inner substs, Ref, Tuple, Array, etc.

**Step 3d** ✅: Created 8 private helpers for walking each MIR construct:
`collect_from_mir_body`, `collect_from_statement`, `collect_from_rvalue`,
`collect_from_aggregate_kind`, `collect_from_operand`, `collect_from_place`,
`collect_from_projection_elem`, `collect_from_terminator`.

**Step 3e** ✅: 24 unit tests + 12 integration tests covering all collection
paths, dedup, nested generics, and no-regression checks.

### 3.4 Phase 4: Per-Mono Codegen 🔧 IN PROGRESS (Stage 16.55 = 4a complete)

**Goal**: Each `MonoItem` gets its own specialized LLVM type/function.

**Step 4a (Stage 16.55)** ✅: Specialized naming scheme
```rust
// In src/mir/monomorphize.rs
pub fn mangle_ty(ty: &Ty) -> String  // "i32", "Adt_5_i32", "ref_i32", etc.
pub fn mangle_ty_with_interner(ty, type_names, interner) -> String  // "Box_i32"
pub fn mono_item_name(item, base_name, type_names, interner) -> String  // "Box_i32"
pub fn build_mono_item_names(items, fn_names, type_names, interner) -> HashMap<MonoItem, String>
```

**Step 4b** 🔧: Layouts keyed by `(DefId, SubstsRef)`
```rust
// Before: HashMap<DefId, AdtLayout>
// After:  HashMap<(DefId, SubstsRef), AdtLayout>
```

**Step 4c** 🔧: Emit specialized function definitions
```llvm
; Before: define i32 @landin_vec_push(...)
; After:  define i32 @landin_vec_push_i32(...)
;         define i32 @landin_vec_push_bool(...)
```

## 4. API Naming (§23)

| Function | Pattern |
|----------|---------|
| `generics_of` | `<noun>_<prep>` — query function |
| `substitute` | `<verb>` — pure function |
| `collect_mono_items` | `<verb>_<noun>_<noun>` — collection pass |
| `MonoItem` | `<Noun>` — data type |
| `lower_generic_args` | `<verb>_<adj>_<noun>` — lowering function |

## 5. Test Plan

### Phase 1 Tests ✅ COMPLETE
- ✅ `let x: Vec<i32>` → MIR type `Adt(Vec_def_id, [i32])` (Stage 16.51)
- ✅ `struct Pair<A, B> { a: A, b: B }` → `Adt(Pair_def_id, [T1, T2])` (Stage 16.51)
- ✅ `let x: Opt<i32> = Opt::Some(42)` → annot substs unify with aggregate (Stage 16.52)
- ✅ `let x: Opt<i32> = Opt::None` → empty substs unify with non-empty (Stage 16.52)
- ✅ Generic enum in return position and match scrutinee (Stage 16.52)
- 🔧 `let x: Vec<Vec<i32>>` → nested substs (Phase 2)

### Phase 2 Tests ✅ COMPLETE (Stage 16.53)
- ✅ `substitute(Param(0), [i32])` → `i32` (29 unit tests)
- ✅ `substitute(Adt(Box, [Param(0)]), [i32])` → `Adt(Box, [i32])` (unit test)
- ✅ `let b: Box<i32> = Box { val: 42 }; b.val` compiles (integration test)
- ✅ `impl<X> S<X> { fn get(&self) -> X { self.x } }` compiles (integration test)
- ✅ `impl<X: T> T for S<X> { fn f(&self) -> i32 { self.x.f() } }` compiles (integration test)
- ✅ Generic struct field access produces concrete type in MIR (MIR inspection test)

### Phase 3 Tests ✅ COMPLETE (Stage 16.54)
- ✅ `collect_mono_items` walks MIR bodies (24 unit tests)
- ✅ `Vec<i32>` + `Vec<bool>` → 2 MonoItems (integration test)
- ✅ `Vec<i32>` + `Vec<i32>` → 1 MonoItem (dedup, integration test)
- ✅ Non-generic code → 0 MonoItems (integration test)
- ✅ Nested generics produce MonoItems (integration test, with known limitation)

### Phase 4a Tests ✅ COMPLETE (Stage 16.55)
- ✅ `mangle_ty(i32)` → `"i32"` (16 unit tests covering all TyKind variants)
- ✅ `mangle_ty(Adt(Box, [i32]))` → `"Adt_5_i32"` (DefId fallback)
- ✅ `mono_item_name(Type{Box, [i32]}, "Box")` → `"Box_i32"` (5 tests)
- ✅ `build_mono_item_names` builds full map (3 tests: basic, empty, mixed)

### Phase 4b-4c Tests
- `Vec<i32>` and `Vec<bool>` produce different LLVM types
- `fn id<T>(x: T) -> T` called with `i32` and `bool` → 2 functions

## 6. References

- Stage 16.49 investigation: `docs/develop/v0/stage-16/stage-16.49-generic-parser-investigation.md`
- Stage 16.50 design (Phase 1a): `docs/develop/v0/stage-16/stage-16.50-generics-of-query.md`
- Stage 16.51 design (Phase 1b): `docs/develop/v0/stage-16/stage-16.51-substs-propagation.md`
- Stage 16.52 design (Phase 1c): `docs/develop/v0/stage-16/stage-16.52-aggregate-substs-propagation.md`
- Stage 16.53 design (Phase 2): `docs/develop/v0/stage-16/stage-16.53-type-substitution.md`
- Stage 16.54 design (Phase 3): `docs/develop/v0/stage-16/stage-16.54-monomorphization-collection.md`
- Stage 16.55 design (Phase 4a): `docs/develop/v0/stage-16/stage-16.55-per-mono-codegen-naming.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md` (Task 11 section)
- Type system data flow: `docs/graph/type-system/data-flow.md`
