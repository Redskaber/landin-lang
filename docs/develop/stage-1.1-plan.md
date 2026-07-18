# Stage 1.1 — HIR Data Structures + Deferred AST Schema Fixes

> **Sub-stage**: 1.1 (Month 3, weeks 1-2)
> **Goal**: Define the HIR data structures needed for Stage 1.2 (AST → HIR
> lowering) and Stage 1.3 (name resolution). Also fix the 3 deferred P0 AST
> schema changes from the Stage 0 v0.1.4 committee review.
> **Acceptance gate**: All 5 Stage Committee members vote APPROVED or
> APPROVED WITH MINOR CONCERNS (≤2 minor).

---

## Tasks (12 atomic items)

### Phase A — Deferred AST schema fixes (must come first; HIR reuses these)

#### A1. `Param.self_kind: Option<SelfKind>` field

**Problem**: `&self`, `&mut self`, `self`, `mut self` produce byte-identical
AST nodes (only the `is_self: bool` flag is set, mutability/receiver-kind
lost). Stage 2 borrow-check cannot tell an immutable-method-receiver from a
mutable one.

**Fix**:

1. Define new enum in `src/ast/kinds.rs`:

   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum SelfKind {
       Value,              // self / mut self
       Ref(Mutability),    // &self / &mut self
   }
   ```

2. Add `pub self_kind: Option<SelfKind>` to `Param` (Some iff `is_self`).
3. Update `parse_params` to populate `self_kind` from the detected
   `&` / `mut` info.
4. Add 4 regression tests asserting different self kinds produce different
   `SelfKind` values.

#### A2. `BindingMode::ByValue(Mutability)` — preserve `mut` binding

**Problem**: `let x = ...` and `let mut x = ...` produce identical
`BindingMode::ByValue` (no mutability payload). Stage 2 cannot enforce
immutable bindings.

**Fix**:

1. Change `BindingMode::ByValue` to `BindingMode::ByValue(Mutability)`.
2. Update all 4 construction sites in `parse_pat_no_or`:
   - `KwMut` arm → `ByValue(Mutability::Mutable)`
   - `KwRef` arm → `ByRef(mutability)` (already correct)
   - Default ident arm → `ByValue(Mutability::Immutable)`
   - Struct pattern shorthand → `ByValue(Mutability::Immutable)`
3. Update `parse_params` self arm to use `ByValue(Mutability::Immutable)`
   (self params don't use `mut` keyword the same way; receiver mutability
   is in `SelfKind`).
4. Update `parse_pat` slice `..` rest default to use `ByValue(Immutable)`.
5. Add 3 regression tests: `let x`, `let mut x`, `let ref mut x` produce
   distinct BindingMode values.

#### A3. Type-position-only generic args heuristic

**Problem**: `try_parse_generic_args` is called from `parse_path` (which is
used in both type and expression positions). In expression position, `a < b`
is misparsed as `Path(a::<b>)` because the heuristic accepts `Ident` after
`<`.

**Fix**:

1. Add `fn try_parse_generic_args_in_type(&mut self) -> Option<GenericArgs>`
   that always tries to parse generic args (the current behavior).
2. Add `fn try_parse_generic_args_in_expr(&mut self) -> Option<GenericArgs>`
   that requires `::<` (turbofish) — i.e., only accept generic args in
   expression position when preceded by `::`.
3. Refactor `parse_path` to take a `PathContext` enum (`Type` / `Expr` /
   `Pattern`) and dispatch to the right variant.
4. Update callers:
   - `parse_ty` → `PathContext::Type`
   - `parse_primary_expr` (path arm) → `PathContext::Expr`
   - `parse_pat_no_or` (path arm) → `PathContext::Pattern` (same as Type)
   - `parse_type_bounds` → `PathContext::Type`
   - `parse_use_tree` → `PathContext::Type` (use paths are like types)
5. Add 4 regression tests: `a < b` parses as comparison; `Vec::<i32>` parses
   as turbofish; `Vec<i32>` in type position parses as generic args;
   `foo::<i32>()` parses as method call with turbofish.

### Phase B — HIR module skeleton

#### B1. Create `src/hir/` directory

```
src/hir/
├── mod.rs       Module root, public API exports
├── id.rs        HirId, DefId, ItemLocalId, OwnerId
├── map.rs       HirIdMap, HirIdSet (typed wrapper around FxHashMap/FxHashSet)
└── kinds.rs     All HIR node type definitions
```

#### B2. `HirId` + `DefId` + `ItemLocalId`

Define in `src/hir/id.rs`:

```rust
/// Identifies a definition (an item or a body).
/// Per-crate monotonically increasing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DefId(pub u32);

/// Local identifier within an owner's body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemLocalId(pub u32);

/// Unique identifier for any HIR node (item or expression/statement/pattern
/// within a body). Two HirIds are equal iff they refer to the same node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HirId {
    pub owner: DefId,
    pub local_id: ItemLocalId,
}

/// Owner of a body — fns, consts, statics have bodies; types/items without
/// bodies (struct decls, enums, traits) are "owners" but don't have a body
/// in the HIR sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnerId(pub DefId);
```

Add `Default` impl for tests. Add `Display` impls for diagnostics.

#### B3. `HirIdMap` / `HirIdSet`

Define in `src/hir/map.rs`:

```rust
use std::collections::HashMap;
use std::collections::HashSet;
use crate::hir::id::HirId;

pub type HirIdMap<V> = HashMap<HirId, V>;
pub type HirIdSet = HashSet<HirId>;
```

(We avoid pulling in `fxhash` for now; std HashMap is fine for Stage 1.)

### Phase C — HIR node definitions

#### C1. `OwnerNodes` — HIR form of all item kinds

Define in `src/hir/kinds.rs`:

```rust
pub enum OwnerNode {
    Item(HirItem),
    ForeignItem(HirForeignItem),
    TraitItem(HirTraitItem),
    ImplItem(HirImplItem),
}

pub enum HirItem {
    Fn(HirFn),
    Const(HirConst),
    Static(HirStatic),
    Struct(HirStruct),
    Enum(HirEnum),
    Trait(HirTrait),
    Impl(HirImpl),
    TypeAlias(HirTypeAlias),
    ExternBlock(HirExternBlock),
    Mod(HirMod),
    Use(HirUse),
}
```

Each variant carries:

- `hir_id: HirId` (or `owner: OwnerId` for top-level)
- `ident: Ident` (preserved from AST)
- `vis: Visibility` (preserved)
- `attrs: Vec<Attr>` (preserved)
- `span: Span` (preserved)
- kind-specific fields (e.g., `HirFn` has `sig: HirFnSig`, `body: Option<BodyId>`)

#### C2. `Body` — function/const/static initializer

```rust
/// A body is the expression/statement tree of a fn/const/static.
/// Stored separately from the owner so that name resolution and type
/// inference can iterate owners first, then bodies.
pub struct Body {
    pub hir_id: HirId,
    pub params: Vec<HirParam>,
    pub value: HirExpr,  // Block for fn; Expr for const/static
    pub span: Span,
}

pub struct HirParam {
    pub hir_id: HirId,
    pub pat: HirPat,
    pub ty: Option<HirTy>,    // None for `self` shorthand
    pub self_kind: Option<SelfKind>,  // Some iff this is a self param
    pub span: Span,
}
```

#### C3. `HirExpr` / `HirStmt` / `HirPat` / `HirTy` / `HirPath`

Each is structurally similar to the AST counterpart but:

- Every node carries `hir_id: HirId`
- `HirTy` has an `inferred: Option<InferTy>` placeholder for Stage 2
- `HirPath` carries `res: Option<Res>` (resolution result; `None` until
  Stage 1.3 name resolution runs)

Define ~28 `HirExpr` variants mirroring `Expr` (Lit, Path, Binary, Call,
MethodCall, Field, Index, Unary, Assign, AddrOf, Cast, Try, If, Match, Loop,
While, For, Closure, Return, Break, Continue, Range, Tuple, Array, Repeat,
Struct, MacroCall, Unsafe, Unit, Block, Let).

Define ~12 `HirPat` variants mirroring `Pat` (Wild, Ident, Struct, TupleStruct,
Tuple, Slice, Or, Path, Lit, Range, Ref, Rest).

Define ~16 `HirTy` variants mirroring `Ty` (Bool, Char, Int, Uint, Float,
Never, Tuple, Array, Slice, Ref, Ptr, FnPtr, Path, TraitObject, ImplTrait,
Infer).

#### C4. `HirGenericParam` / `HirWherePredicate`

Migrate from AST but stricter: every node carries `HirId`.

```rust
pub struct HirGenerics {
    pub params: Vec<HirGenericParam>,
    pub where_clause: Vec<HirWherePredicate>,
    pub span: Span,
}

pub enum HirGenericParam {
    Lifetime(HirLifetimeParam),
    Type(HirTypeParam),
}

pub struct HirTypeParam {
    pub hir_id: HirId,
    pub ident: Ident,
    pub bounds: Vec<HirTypeBound>,
    pub default: Option<HirTy>,
    pub span: Span,
}
```

#### C5. `InferTy` placeholder

```rust
/// Placeholder for a type that will be inferred by Stage 2 typeck.
/// During HIR construction we create fresh InferTy vars; typeck will
/// unify them with concrete types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InferTy(pub u32);

/// A counter for generating fresh InferTy vars. Per-crate.
pub struct InferTyCounter {
    next: u32,
}

impl InferTyCounter {
    pub fn new() -> Self { Self { next: 0 } }
    pub fn fresh(&mut self) -> InferTy { let v = self.next; self.next += 1; InferTy(v) }
}
```

#### C6. `Res` — name resolution result placeholder

```rust
/// The resolution of a path. Populated by Stage 1.3 name resolution.
/// During HIR construction, all paths have `Res::Unknown`; the resolver
/// fills in the actual target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Res {
    Unknown,
    Local(HirId),
    Def(DefId),
    PrimTy(PrimTy),
    SelfTy,
    SelfCtor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimTy {
    Bool, Char, I8, I16, I32, I64, I128, Isize,
    U8, U16, U32, U64, U128, Usize,
    F32, F64,
    Str,
}
```

### Phase D — Tests + docs

#### D1. HIR unit tests (30+ tests in `tests/hir_structure.rs`)

- 5 tests: `HirId` / `DefId` / `ItemLocalId` construction, equality, ordering
- 5 tests: `HirIdMap` / `HirIdSet` insertion, lookup, iteration
- 5 tests: `InferTy` counter freshness
- 5 tests: each `HirItem` variant construction + Debug output
- 5 tests: `Body` construction with params + value
- 5 tests: `HirExpr` / `HirPat` / `HirTy` representative variant construction

#### D2. AST regression tests (12 tests in `tests/ast_structure.rs`)

- 4 tests for A1 (self kind preservation)
- 3 tests for A2 (binding mode mutability preservation)
- 5 tests for A3 (type-vs-expr generic args disambiguation)

#### D3. Documentation updates

- Update `docs/development-log.md` with Stage 1.1 progress
- Update `docs/stage0-status.md` → supersede by `docs/stage1-status.md`
- Update `README.md` to mention Stage 1.1 HIR skeleton
- Update `Cargo.toml` version: v0.1.4 → v0.2.0

---

## Acceptance Criteria

A task is "done" iff:

1. ✅ All listed sub-items implemented and committed
2. ✅ `cargo build` produces 0 warnings
3. ✅ `cargo clippy --all-targets -- -D warnings` passes
4. ✅ `cargo fmt --check` passes
5. ✅ `cargo test` passes with ≥360 tests (330 existing + 30 new HIR + 12 new AST regression = 372)
6. ✅ HIR data structures are complete enough that Stage 1.2 (AST → HIR
   lowering) can begin without further schema changes
7. ✅ All 5 Stage Committee members vote APPROVED or APPROVED WITH MINOR CONCERNS

---

## Risk Assessment

- **A1-A3 are AST schema changes** that touch many call sites. Risk: breaking
  existing 330 tests. Mitigation: run tests after each sub-task.
- **HIR module is large** (~30 enum variants, ~50 structs). Risk: design
  drift from AST. Mitigation: 70% structural isomorphism with AST is the
  design goal; document deviations.
- **`Res` placeholder** is forward-looking. Risk: over-engineering for Stage 1.
  Mitigation: only define the enum; do not implement resolution logic.

## Time estimate

- Phase A (AST fixes): 1-2 hours
- Phase B (HIR skeleton): 30 minutes
- Phase C (HIR nodes): 2-3 hours
- Phase D (tests + docs): 1-2 hours
- Self-review + committee: 1 hour
- **Total**: 5-8 hours of focused work

---

**This plan is the contract for Stage 1.1. Deviations require a new plan
and re-approval.**
