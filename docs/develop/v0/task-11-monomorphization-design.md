# Task 11 Design Document — Monomorphization (Generic Support)

> **Author**: redskaber
> **Date**: 2026-08-04 (Stage 16.49)
> **Version**: v0.238.0 (Stage 16.52)
> **Process**: stage-committee-process.md v3.24 §13.4 (stage-start design alignment)
> **Status**: Phase 1 complete (1a + 1b + 1c), Phase 2-4 planned

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

### 3.2 Phase 2: Substitution 🔧 NEXT

**Goal**: Given `struct Vec<T> { data: [T; N], len: usize }` and substs `[i32]`,
produce field type `[i32; N]` for the `data` field.

```rust
// In src/mir/ty.rs
pub fn substitute(ty: &Ty, substs: &[Ty]) -> Ty {
    match &ty.kind {
        TyKind::Param(ParamTy { index, .. }) => substs[*index as usize].clone(),
        TyKind::Adt(def_id, inner_substs) => {
            let substituted = inner_substs.iter().map(|t| substitute(t, substs));
            Ty::new(TyKind::Adt(*def_id, substituted.collect::<Vec<_>>().into()), ty.span)
        }
        // ... recursively handle Ref, Tuple, Array, etc.
    }
}
```

### 3.3 Phase 3: Monomorphization Collection

**Goal**: Walk all MIR bodies, collect `MonoItem { def_id, substs }` pairs,
dedup.

```rust
// In src/mir/monomorphize.rs
pub struct MonoItem {
    pub def_id: DefId,
    pub substs: SubstsRef,
}

pub fn collect_mono_items(mirs: &[MirBody]) -> Vec<MonoItem> {
    // Walk all types and function calls
    // For each Adt(def_id, substs) where !substs.is_empty(): collect
    // For each FnDef(def_id, substs) where !substs.is_empty(): collect
    // Recursively substitute into generic function bodies
    // Dedup by (def_id, substs)
}
```

### 3.4 Phase 4: Per-Mono Codegen

**Goal**: Each `MonoItem` gets its own specialized LLVM type/function.

**Layouts**: Keyed by `(DefId, SubstsRef)` instead of just `DefId`.
```rust
// Before: HashMap<DefId, Vec<Ty>>
// After: HashMap<(DefId, SubstsRef), Vec<Ty>>
```

**Functions**: Emit `landin_<name>_<mono_hash>` for each `MonoItem`.
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

### Phase 2 Tests
- `substitute(Vec<T>.data, [i32])` → `[i32; N]`
- `substitute(Pair<A, B>.a, [i32, bool])` → `i32`

### Phase 3 Tests
- `Vec<i32>` + `Vec<bool>` → 2 MonoItems
- `Vec<i32>` + `Vec<i32>` → 1 MonoItem (dedup)

### Phase 4 Tests
- `Vec<i32>` and `Vec<bool>` produce different LLVM types
- `fn id<T>(x: T) -> T` called with `i32` and `bool` → 2 functions

## 6. References

- Stage 16.49 investigation: `docs/develop/v0/stage-16/stage-16.49-generic-parser-investigation.md`
- Stage 16.50 design (Phase 1a): `docs/develop/v0/stage-16/stage-16.50-generics-of-query.md`
- Stage 16.51 design (Phase 1b): `docs/develop/v0/stage-16/stage-16.51-substs-propagation.md`
- Stage 16.52 design (Phase 1c): `docs/develop/v0/stage-16/stage-16.52-aggregate-substs-propagation.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md` (Task 11 section)
- Type system data flow: `docs/graph/type-system/data-flow.md`
