# Stage 1.3 — Module-Level Name Resolution

> **Sub-stage**: 1.3 (Month 4, weeks 1-2)
> **Goal**: Walk all `HirPath` nodes in the HIR crate and replace
> `Res::Unknown` with the appropriate `Res::{Def, Local, PrimTy, SelfTy, ...}`
> based on module-level name resolution.
> **Acceptance gate**: All 5 Stage Committee members vote APPROVED (strict).
> **Process**: 4-10 rounds of review-refine-iterate.

---

## Scope

Stage 1.3 handles **module-level** resolution only:
- Top-level item registration (fn/const/static/struct/enum/trait/impl/type/mod/use)
- `use` declaration resolution (simple `use a::b::c;`, glob `use a::*;`,
  group `use a::{b, c};`, alias `use a::b as c;`)
- Path resolution (`a::b::c` → `Res::Def(DefId)`)
- Bare identifier resolution (in type/value position)
- Primitive type recognition (`i32`, `bool`, etc. → `Res::PrimTy`)
- `Self` type resolution in impl/trait context
- `crate`/`super`/`self` path prefixes
- Visibility checking (`pub` / `pub(crate)` / `pub(super)` / `pub(in path)`)
- Duplicate definition detection
- Prelude injection (implicit `use std::prelude::v1::*`)

**NOT in scope** (deferred to Stage 1.4 — scope-based resolution):
- `let` binding resolution within function bodies
- Closure parameter resolution
- Match arm pattern binding resolution
- Forward reference detection within fn bodies
- Shadowing detection
- Label resolution (`'lbl:` for loop / break 'lbl)

---

## Tasks (14 atomic items across 5 phases)

### Phase A — Resolver infrastructure

#### A1. `Resolver` struct + `ResolveCtxt`

Create `src/resolve/mod.rs`:

```rust
pub struct Resolver {
    /// Module tree: crate root → nested mods
    module_tree: ModuleNode,
    /// Map from DefId → DefKind (for namespace disambiguation)
    def_kinds: HashMap<DefId, DefKind>,
    /// Errors encountered (non-fatal)
    errors: Vec<ResolveError>,
    /// The HIR crate being resolved (mutated in-place on HirPath.res)
    hir: HirCrate,
}
```

#### A2. `DefKind` enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    Fn,
    Const,
    Static,
    Struct,
    Enum,
    Trait,
    Impl,
    TypeAlias,
    Mod,
    Use,
    ExternFn,
    ExternStatic,
    ExternType,
}
```

#### A3. `ModuleNode` — module tree

```rust
#[derive(Debug, Clone, Default)]
pub struct ModuleNode {
    /// Items in the value namespace (fn, const, static)
    value_ns: HashMap<Symbol, DefId>,
    /// Items in the type namespace (struct, enum, trait, type alias, mod)
    type_ns: HashMap<Symbol, DefId>,
    /// Child modules (by name)
    children: HashMap<Symbol, ModuleNode>,
    /// Visibility of this module (from parent)
    vis: Visibility,
    /// Parent module (None for crate root)
    parent: Option<Symbol>,
}
```

#### A4. `ResolveError`

```rust
#[derive(Debug, Clone)]
pub struct ResolveError {
    pub message: String,
    pub span: Span,
    pub code: Option<&'static str>, // e.g. "E0401" — future
}
```

### Phase B — Module tree construction

#### B1. Build module tree from HIR crate

Walk all owners in the HIR crate and build the module tree:
- Crate root: all top-level items
- `mod foo { ... }`: create child module `foo`, recursively walk its items
- `mod foo;` (out-of-line): create placeholder child module `foo`
- Register each item in the appropriate namespace (value/type) of its
  containing module

#### B2. Register `DefKind` for each DefId

Walk all owners and record `DefId → DefKind` mapping. This is used
for namespace disambiguation during path resolution.

### Phase C — Use declaration resolution

#### C1. Process `use` declarations

For each `HirUse` in the module tree:
- `UseTree::Leaf(path, alias)` — resolve `path` to a `DefId`, then
  import the name (alias or last segment) into the current module's
  appropriate namespace
- `UseTree::Glob(path)` — resolve `path` to a module, then import all
  its public items into the current module
- `UseTree::Path { prefix, children }` — recursively process children
  with `prefix` as the base path

#### C2. Handle use ambiguities

- Duplicate imports (same name imported twice) → error
- Name conflicts with local definitions → error
- Glob imports that shadow explicit imports → explicit wins (per Rust)

### Phase D — Path resolution

#### D1. `resolve_path` — main resolution function

```rust
fn resolve_path(&self, path: &HirPath, ctx: &ResolveCtx) -> Res {
    // Handle path leading: Root / Crate / Super / Self_ / None
    // Walk segments left-to-right
    // First segment: look up in current module's appropriate namespace
    // Subsequent segments: look up in the resolved module's children
    // Return Res::Def(DefId) or Res::Err
}
```

#### D2. Bare identifier resolution

In type position: `i32` → `Res::PrimTy(PrimTy::I32)`, `Foo` →
look up in type namespace.

In value position: `foo` → look up in value namespace (will be
`Res::Local(HirId)` for locals — but locals are Stage 1.4; for
Stage 1.3 we only resolve to `Res::Def`).

#### D3. Primitive type recognition

Recognize all 16 primitive types by name:
`bool`, `char`, `i8`-`i128`, `isize`, `u8`-`u128`, `usize`, `f32`,
`f64`, `str`.

#### D4. `Self` type resolution

In an `impl Foo { ... }` block: `Self` → the impl's `self_ty`.
In a `trait Foo { ... }` block: `Self` → `Res::SelfTy` (the trait's
Self type parameter).

#### D5. Walk HIR and fill `Res` on every `HirPath`

After module tree + use resolution is complete, walk every `HirPath`
in the HIR crate and call `resolve_path` to fill in `res`.

### Phase E — Visibility + duplicates + prelude

#### E1. Visibility checking

After resolution, verify that each resolved `DefId` is visible from
the current module:
- `pub` → visible everywhere
- `pub(crate)` → visible within the crate
- `pub(super)` → visible in parent module
- `pub(in path)` → visible within `path`
- private → visible only in defining module

#### E2. Duplicate definition detection

During module tree construction, detect:
- Two items with the same name in the same namespace → error
- Two glob imports that bring in the same name → ambiguous (error
  only if both are used)

#### E3. Prelude injection

Before processing user `use` declarations, inject an implicit:
```landin
use std::prelude::v1::*;
```
at the crate root. For Stage 1.3, we don't have a std crate yet, so
this is a no-op placeholder — but the infrastructure is in place for
Stage 5.

### Phase F — Tests + integration

#### F1. Integration: all 413 existing tests pass

Lowering tests should still pass (they don't check `Res` values).
New resolution tests verify `Res` is filled correctly.

#### F2. Resolution structural tests (30+)

- 5 tests: simple name resolution (fn/struct/enum/trait/type)
- 5 tests: path resolution (`a::b::c`)
- 5 tests: use declaration resolution (simple/glob/group/alias)
- 5 tests: primitive type resolution
- 3 tests: Self type resolution
- 3 tests: visibility checking
- 2 tests: duplicate definition detection
- 2 tests: crate/super/self path prefixes

#### F3. Documentation

- `docs/stage-1.3-plan.md`: this plan
- `README.md`: bump to v0.2.2
- `Cargo.toml`: v0.2.1 → v0.2.2

---

## Acceptance Criteria

1. ✅ All 14 tasks implemented
2. ✅ `cargo build` 0 warnings
3. ✅ `cargo clippy --all-targets -- -D warnings` passes
4. ✅ `cargo fmt --check` passes
5. ✅ `cargo test` passes with ≥443 tests (413 existing + 30 new)
6. ✅ All `HirPath.res` fields are populated (no `Res::Unknown` remaining
   after resolution, except for genuinely unresolved paths which get
   `Res::Err`)
7. ✅ All 5 Stage Committee members vote APPROVED (strict)
