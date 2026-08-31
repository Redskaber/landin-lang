# Resolve Data Flow (HIR with Res::Unknown → HIR with Res::*)

> **Date**: 2026-08-31
> **Version**: v0.557.0
> **Stage**: 30.22 (graph docs gap closure)

## Module Overview

The name resolver walks all `HirPath` nodes in the HIR crate and
replaces `Res::Unknown` (set by HIR lower) with the appropriate
`Res::{Def, Local, PrimTy, SelfTy, Err}`. Per Stage 1.3 plan, the
resolver handles **module-level** resolution only (items, use imports,
path resolution, primitives, Self). Local variable resolution within
bodies is Stage 1.4 (scope-based via `ScopeStack`).

Per 01-language-specification.md §6.2 the resolution order is split
into 5 passes: pass 1 builds the module tree; passes 2-3 resolve use
imports; pass 4 resolves paths in items/signatures; pass 5 resolves
paths in bodies. Stage 6.16 (TD-026) split the resolver into 3
sibling sub-modules: `module_build.rs` (passes 1-3),
`path_resolve.rs` (passes 4-5), `primitives.rs` (primitive type
lookup table). Stage 26.1 (v0.8) added real visibility enforcement
via `def_visibility` + `def_owner_module` maps.

## Data Flow Diagram

```
HirCrate (from hir::lower, all Res = Res::Unknown)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  Resolver::new()                     src/resolve/resolver.rs│
│                                                               │
│  module_tree: ModuleNode (crate root → nested mods)          │
│  def_kinds: HashMap<DefId, DefKind> (namespace disambiguation)│
│  def_visibility: HashMap<DefId, Visibility>                   │
│  def_owner_module: HashMap<DefId, Spur> (which mod owns it)   │
│  def_span: HashMap<DefId, Span> (Stage 18.57, accurate errors)│
│  scopes: Option<ScopeStack> (None at crate level)              │
│  current_self_kind: Option<HirSelfKind> (for Self resolution) │
│  current_module: Option<Spur> (for visibility enforcement)    │
│  impl_method_index: HashMap<(Spur, Spur), DefId>              │
└─────────────┬────────────────────────────────────────────────┘
              │ resolve_crate(&mut hir, &interner)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Pass 1-3: Module tree + use imports                        │
│  (src/resolve/module_build.rs)                                │
│                                                               │
│  build_module_tree(&hir):                                     │
│    - Walk owners, register DefId → DefKind + Visibility      │
│    - Build nested ModuleNode tree (HirMod::Inline vs Items)   │
│    - Populate def_visibility + def_owner_module + def_span   │
│    - Build impl_method_index ((type, method) → DefId)         │
│      (Stage 14.41 — fixes `V::new` resolving to struct ctor) │
│                                                               │
│  resolve_use_imports(&mut hir):                               │
│    - Walk HirUse trees, map to UseImport::Single/Alias/Glob  │
│    - Add to module tree for name lookup                       │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Pass 4: Item/signature paths                                │
│  (src/resolve/path_resolve.rs)                                │
│                                                               │
│  for owner in hir.owners:                                     │
│    resolve_owner_paths(owner):                                │
│      - HirFn return type, param types                          │
│      - HirStruct/HirEnum field types                          │
│      - HirImpl self_ty + trait_ref + method sigs              │
│      - HirTrait supertraits + associated types                │
│      - HirUse tree targets                                    │
│      - HirStatic/HirConst ty                                  │
│      - HirTypeAlias ty                                        │
│    resolve_path(HirPath):                                     │
│      1. Check impl_method_index for `Type::method`            │
│      2. Check current_module tree for `Item`                  │
│      3. Check use imports                                      │
│      4. Check primitives (src/resolve/primitives.rs)          │
│      5. Check Self (current_self_kind)                       │
│      6. Else: Res::Err + ResolveError                         │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Pass 5: Body paths (Stage 1.4)                              │
│                                                               │
│  for body in hir.bodies:                                      │
│    scopes = Some(ScopeStack::new())                          │
│    walk body:                                                  │
│      - HirLocal binding → push to scope                       │
│      - HirPath::Local → lookup in ScopeStack                  │
│      - HirExpr Match arm patterns → push bindings             │
│      - HirClosure params → push to scope                      │
│      - On block exit → pop scope                              │
└─────────────┬────────────────────────────────────────────────┘
              │ Vec<ResolveError>
              ▼
              → CompileErrors.resolve (non-fatal — HIR still produced)
              → mir::lower::lower_hir_body_to_mir reads Res for path lower
              → traits::TraitResolver::collect reads HirItem::Trait/Impl
```

## Key Data Structures

- **`Resolver`** (`src/resolve/resolver.rs`) — Holds `module_tree`,
  `def_kinds`, `def_visibility`, `def_owner_module`, `def_span`,
  `scopes`, `current_self_kind`, `current_module`,
  `impl_method_index`. Built per-crate.
- **`ModuleNode`** (`src/resolve/module_tree.rs`) — Tree node:
  `{ name: Spur, kind: ModuleKind, children: Vec<ModuleNode>,
  items: Vec<DefId>, use_decls: Vec<UseDecl> }`. Forms the
  namespace for path resolution.
- **`Scope` / `ScopeStack` / `ScopeKind`** (`src/resolve/scope.rs`)
  — Local-variable scope for body resolution (Stage 1.4). `ScopeStack`
  is a stack of `Scope`s; `ScopeKind` differentiates Fn / Block /
  MatchArm / Closure.
- **`Res`** (`src/hir/kinds.rs`, re-exported via `crate::hir::Res`)
  — `Unknown | Def(DefId, DefKind) | Local(HirId) | PrimTy(PrimTy) |
  SelfTy(HirSelfKind) | Err`. Filled by resolve.
- **`ResolveError` / `ResolveErrorKind`** (`src/resolve/error.rs`)
  — Structured error: `UnresolvedName`, `AmbiguousName`,
  `VisibilityViolation` (Stage 26.1), `DuplicateDefinition`,
  `InvalidUseTree`. Surfaced via `CompileErrors.resolve` (non-fatal).
- **`UseDecl` / `UseImport`** (`src/resolve/module_tree.rs`) —
  Use-import entries in module tree: `UseImport::Single { path,
  alias } | Glob { path } | Alias { path, alias }`. Used by
  `resolve_path` to expand `use` statements.

## Dependencies

**Upstream inputs:**
- `&mut HirCrate` from `hir::lower` (mutated in place — every
  `HirPath.res` field filled in).
- `&Rodeo` interner for symbol comparison during path lookup.

**Downstream consumers:**
- `src/mir/lower/path.rs` — reads `Res::Def(def_id, kind)` to decide
  whether a path is a struct ctor, fn, const, etc.
- `src/traits/resolver.rs` — reads `HirItem::Trait` / `HirItem::Impl`
  to build TraitInfo / ImplInfo (the resolver pass must complete first
  so paths in trait bounds + impl headers are resolved).
- `src/driver/mod.rs` — calls `resolve_crate` before
  `traits::collect` and `mir::lower`; collects `ResolveError` into
  `CompileErrors.resolve`.

## Stage Boundaries

Per §16 (interface isolation), resolve is the only pass that mutates
HIR after the lower phase. It reads HIR + writes back `Res` fields;
all later passes treat HIR as read-only data. Resolve sits at pipeline
position 4 (after HIR lower, before traits collect + MIR lower).
The 3-way split (Stage 6.16 TD-026) follows §14.4 (refactoring as
architecture design) aligned with 01-language-specification.md §6.2
resolution order. The Stage 14.41 `impl_method_index` (maps
`(type_name, method_name) → DefId`) eliminated the long-standing bug
where `V::new(1, 2)` was treated as a struct constructor instead of a
method call. Stage 26.1 (v0.8) wired `def_visibility` +
`def_owner_module` into `check_visibility` for real pub/private
enforcement — previously a stub since the module tree was flat.
