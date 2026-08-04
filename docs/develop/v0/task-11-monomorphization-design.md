# Task 11 Design Document — Monomorphization (Generic Support)

> **Author**: redskaber
> **Date**: 2026-08-04 (Stage 16.49)
> **Version**: v0.236.0
> **Process**: stage-committee-process.md v3.24 §13.4 (stage-start design alignment)
> **Status**: Design phase — investigation complete, implementation planned

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

### 3.1 Phase 1: Substs Propagation

**Goal**: `let x: Vec<i32>` produces `Adt(Vec_def_id, [i32])` in MIR.

**Step 1a**: Create `generics_of` query
```rust
// In src/hir/ or src/typeck/
pub fn generics_of(hir: &HirCrate, def_id: DefId) -> Vec<ParamTy>
```
Walks HIR items, collects type parameters into a `DefId → Vec<ParamTy>` map.

**Step 1b**: Modify `lower_hir_ty_to_mir_ty`
```rust
// Before:
TyKind::Adt(def_id, Vec::new().into())

// After:
let substs = lower_generic_args(path.segments.last().args, hir);
TyKind::Adt(def_id, substs.into())
```

**Step 1c**: Same for `AggregateKind::Adt` in struct/enum literal construction.

### 3.2 Phase 2: Substitution

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

### Phase 1 Tests
- `let x: Vec<i32>` → MIR type `Adt(Vec_def_id, [i32])`
- `let x: Vec<Vec<i32>>` → nested substs
- `struct Pair<A, B> { a: A, b: B }` → `Adt(Pair_def_id, [T1, T2])`

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
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md` (Task 11 section)
- Type system data flow: `docs/graph/type-system/data-flow.md`
