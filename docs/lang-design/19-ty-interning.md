# 19 — Ty Interning Design (v0.2 Phase 1 Task 1)

> **Author**: redskaber
> **Date**: 2026-07-31
> **Status**: Draft (v0.2 Phase 1)

## 1. Problem Statement

The current `Ty` struct is:
```rust
pub struct Ty {
    pub kind: TyKind,
    pub span: Span,
}
```

`TyKind` contains `Box<Ty>` (for Ref, RawPtr, Array, Slice, Const) and
`Vec<Ty>` (for Tuple, Sig.inputs, SubstsRef). This means:

1. **`Ty` is not `Copy`** — every type usage requires `.clone()`
2. **~149 unnecessary clones** of `Ty` across `src/mir` + `src/typeck`
3. **3-5× memory overhead** vs rustc-style interning
4. **Heap allocation per type node** — `Box<Ty>` allocates on every construction
5. **No type deduplication** — `i32` is a different allocation each time

## 2. Design: Arena-Interned Ty

### 2.1 Core Type

```rust
/// An interned type. 8 bytes (pointer to arena-allocated TyData).
/// Copy + Eq + Hash — can be used as HashMap key without cloning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ty<'tcx>(&'tcx TyData<'tcx>);

/// The actual type data, allocated in an arena.
#[derive(Debug)]
struct TyData<'tcx> {
    kind: TyKind<'tcx>,
    span: Span,
}

/// All type kinds (now using Ty<'tcx> instead of Box<Ty>).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TyKind<'tcx> {
    Bool,
    Char,
    Int(IntTy),
    // ... primitives unchanged ...
    Ref(Region, Mutability, Ty<'tcx>),   // was Box<Ty>
    RawPtr(Mutability, Ty<'tcx>),         // was Box<Ty>
    Array(Ty<'tcx>, &'tcx Const<'tcx>),   // was Box<Ty>, Box<Const>
    Slice(Ty<'tcx>),                       // was Box<Ty>
    Tuple(&'tcx [Ty<'tcx>]),               // was Vec<Ty>
    FnDef(DefId, &'tcx [Ty<'tcx>]),        // was SubstsRef = Vec<Ty>
    FnPtr(&'tcx Sig<'tcx>),                // was Box<Sig>... actually Sig contains Vec
    Closure(DefId, &'tcx [Ty<'tcx>]),
    Adt(DefId, &'tcx [Ty<'tcx>]),
    // ...
}
```

### 2.2 Arena (TypeInterner)

```rust
/// The type interner: allocates TyData in a typed-arena and deduplicates.
pub struct TypeInterner<'tcx> {
    arena: typed_arena::Arena<TyData<'tcx>>,
    // Deduplication map: (TyKind, Span) → Ty<'tcx>
    // For v0.2 we can skip Span in the key (Span doesn't affect type identity)
    dedup: FxHashMap<TyKind<'tcx>, Ty<'tcx>>,
}
```

### 2.3 Migration Strategy

Since this is a massive refactoring (every `Ty` usage needs a lifetime parameter),
we'll use a **phased approach**:

**Phase 1a** (this task): Introduce `TypeInterner` + interned `Ty` as a new type,
keep old `Ty` as `LegacyTy` for gradual migration.

**Phase 1b** (follow-up): Migrate all call sites from `LegacyTy` to new `Ty<'tcx>`.

**Phase 1c** (cleanup): Remove `LegacyTy`.

### 2.4 Pragmatic v0.2 Approach

Given the massive scope of adding lifetime parameters to every function that
touches `Ty`, we'll take a **pragmatic shortcut** for v0.2 Phase 1:

Instead of full `Ty<'tcx>` interning, we'll:

1. **Make `Ty` `Copy`** by replacing `Box<Ty>` with `Ty` directly in `TyKind`
   (using `#[derive(Clone, Copy)]` — this requires `Ty` to be small enough)
2. **Use `SmallVec` or inline storage** for `Vec<Ty>` cases (Tuple, SubstsRef)
3. **Skip arena interning** for now — just make `Ty` `Copy` to eliminate clones

This gives us 80% of the benefit (no more `.clone()` calls) with 20% of the
effort (no lifetime parameters everywhere). Full arena interning can be done
in v0.3.

### 2.5 Size Analysis

Current `Ty` size:
- `TyKind` (enum, largest variant = `FnDef(DefId, Vec<Ty>)` = 8 + 24 = 32 bytes)
- `Span` = 8 bytes (lo + hi, each u32)
- Total `Ty` = ~40 bytes (with Box) → too large for Copy

With `Box<Ty>` → `Ty` directly (recursive), `Ty` becomes self-referential.
This doesn't work without indirection.

**Conclusion**: True `Copy` for `Ty` requires either:
1. Arena interning (Ty is a pointer — 8 bytes) ← full solution
2. `Rc<TyKind>` (Ty is a ref-counted pointer — 8 bytes) ← simpler

### 2.6 Decision: Use `Rc<TyKind>` as stepping stone

For v0.2 Phase 1, we'll use `Rc<TyKind>` to make `Ty` `Copy` (8 bytes)
without the complexity of lifetime parameters:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Ty(pub Rc<TyKind>);

// Ty is now cheap to clone (Rc::clone is just incrementing a refcount)
// In v0.3, replace Rc with arena interning (&'tcx TyKind)
```

Wait — `Rc` is not `Copy`. We need `Clone` but not `Copy` for `Rc`.

**Revised decision**: Use `Rc<TyKind>` with `#[derive(Clone)]`. This eliminates
heap allocation for repeated types (Rc deduplication) and makes `.clone()` cheap
(refcount increment vs deep clone). Not `Copy`, but `Clone` is O(1).

Actually, the simplest approach that gives the most benefit: **just remove `Span`
from `Ty` and make `TyKind` use `Rc` for recursive types**. Span should be on
the `Statement` / `LocalDecl`, not on every `Ty`.

## 3. Final Design Decision

For v0.2 Phase 1 Task 1, we'll make the following changes:

1. **Remove `Span` from `Ty`** — Span belongs on `LocalDecl` and `Statement`,
   not on the type itself. This simplifies `Ty` to just `TyKind`.
2. **Replace `Box<Ty>` with `Rc<Ty>` in `TyKind`** — makes `.clone()` O(1)
3. **Replace `Vec<Ty>` with `Rc<[Ty]>`** in `TyKind` — shared slice, O(1) clone
4. **Replace `Box<Const>` with `Rc<Const>`** — same benefit
5. **Replace `Box<Sig>` with `Rc<Sig>`** — same benefit

This gives:
- O(1) `Ty::clone()` (Rc refcount increment)
- Type deduplication potential (same `i32` can share Rc)
- No lifetime parameters needed
- Foundation for v0.3 arena interning (just replace `Rc` with `&'tcx`)

**Effort**: ~1 week (update TyKind + all construction sites + remove span from Ty)
