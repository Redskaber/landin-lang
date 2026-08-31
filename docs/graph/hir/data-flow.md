# HIR Data Flow (AST → HIR)

> **Date**: 2026-08-31
> **Version**: v0.557.0
> **Stage**: 30.22 (graph docs gap closure)

## Module Overview

HIR (High-level Intermediate Representation) is the per-node-ID-annotated
form of the AST. Per 06-mir.md §3, HIR is ~70% structurally isomorphic to
the AST with four key differences: (1) every node carries a `HirId`;
(2) every `HirPath` carries a `res: Res` field (populated by Stage 1.3
name resolution, `Res::Unknown` until then); (3) every `HirTy` carries
an `inferred: Option<InferTy>` field (populated by Stage 2 typeck);
(4) `Body` is split out from owners so name resolution and typeck can
iterate owners first, then descend into bodies.

Stage 1.1 only DEFINES the HIR node structures; Stage 1.2 implements
AST→HIR lowering (`src/hir/lower/`); Stage 1.3 fills `Res`; Stage 2
fills `InferTy`. HIR is the input to MIR lower (Stage 2.2), borrowck
(via HirId), and the trait resolver (collects trait/impl metadata).

## Data Flow Diagram

```
crate::ast::Crate  (from parser)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  HirLowerCtxt::new(interner)          src/hir/lower/cx.rs   │
│                                                               │
│  owners: Vec<OwnerNode>      (Stage 1.2 flat list)            │
│  bodies: HashMap<BodyId, Body>                                │
│  hir_id_counter: HirId      (monotonic per crate)             │
│  def_id_counter: DefId      (per owner)                       │
│  errors: Vec<LowerError>                                     │
└─────────────┬────────────────────────────────────────────────┘
              │ lower_crate(ast, interner)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Walk AST in pre-order (src/hir/lower/item.rs)               │
│                                                               │
│  for item in ast.items:                                       │
│    lower_item(item) → OwnerNode + DefId                       │
│    ├─ Fn      → HirFn { generics, sig, body: BodyId }        │
│    ├─ Struct  → HirStruct { fields, generics }                │
│    ├─ Enum    → HirEnum { variants, generics }                │
│    ├─ Trait   → HirTrait { items, supertraits, generics }    │
│    ├─ Impl    → HirImpl { self_ty, trait?, items, generics } │
│    ├─ Mod     → HirMod { kind: ModKind(Item/Inline) }         │
│    ├─ Use     → HirUse { tree }                              │
│    ├─ Const   → HirConst { ty, body }                        │
│    ├─ Static → HirStatic { ty, body, mutability }            │
│    └─ Type    → HirTypeAlias { ty, generics }                │
│                                                               │
│  Sub-modules add lowering for sub-trees:                      │
│    body.rs   → lower_block, lower_stmt, lower_expr            │
│    ty.rs     → lower_ty (QSelf, generics, refs, tuples)      │
│    pat.rs    → lower_pat (struct/tuple/wild/ident)            │
│    path.rs   → lower_path (sets res = Res::Unknown)           │
│    generics.rs → lower_generics, lower_where_clause            │
└─────────────┬────────────────────────────────────────────────┘
              │ (HirCrate, Vec<LowerError>)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  HirCrate (src/hir/kinds.rs)                                  │
│                                                               │
│  owners: Vec<OwnerNode>                                       │
│  bodies: HashMap<BodyId, Body>                                │
│  All Res fields: Res::Unknown (Stage 1.3 fills)              │
│  All InferTy fields: None (Stage 2 fills)                    │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
              → resolve::resolve_crate(hir, interner)  [fills Res]
              → traits::TraitResolver::collect(&hir)   [reads items]
              → mir::lower::lower_hir_body_to_mir(body, ..)
```

## Key Data Structures

- **`HirCrate`** (`src/hir/kinds.rs`) — Root: `{ owners: Vec<OwnerNode>,
  bodies: HashMap<BodyId, Body> }`. Owners and bodies are split so
  typeck/borrowck can iterate owners first.
- **`HirId` / `DefId` / `OwnerId` / `ItemLocalId`** (`src/hir/id.rs`)
  — Per-node stable identifiers mirroring rustc's design. `HirId` keys
  typeck tables and borrow info; `DefId` identifies owners.
- **`Res`** (`src/hir/kinds.rs`) — Name-resolution result attached to
  every `HirPath`: `Unknown | Def(DefId, DefKind) | Local(HirId) |
  PrimTy(PrimTy) | SelfTy(HirSelfKind) | Err`. Filled by Stage 1.3.
- **`InferTy` / `InferTyCounter`** (`src/hir/kinds.rs`) — Inference
  placeholder attached to every `HirTy` (`Option<InferTy>`); filled by
  Stage 2 typeck, consumed by HIR-based diagnostics.
- **`LowerError`** (`src/hir/lower/error.rs`) — Structured error type
  for malformed AST nodes (e.g. unsupported syntax, missing fields);
  collected into `CompileErrors.lower` (non-fatal, Stage 18.75 P0-1).

## Dependencies

**Upstream inputs:**
- `crate::ast::Crate` from the parser.
- `&Rodeo` (interner) for symbol lookups during path lowering.

**Downstream consumers:**
- `src/resolve/resolver.rs` — fills `Res` on every `HirPath`.
- `src/traits/resolver.rs` — collects `TraitInfo`, `ImplInfo`, vtables.
- `src/mir/lower/mod.rs` — lowers each `Body` to `MirBody`.
- `src/driver/mod.rs` — orchestrates lower_crate; collects LowerError.

## Stage Boundaries

Per §16 (interface isolation), the HIR is the canonical data structure
that all later passes share. MIR lower reads HIR (allowed — data flows
downstream), typeck does NOT read HIR directly (it consumes a
pre-computed `FieldTyTable` from the driver — Stage 18.60 closure),
borrowck reads MIR + resolver-backed Copy info, and codegen reads MIR
only. HIR sits at pipeline position 3 (after parser, before resolve).
The 6-file lowering split (`body`, `cx`, `error`, `generics`, `item`,
`pat`, `path`, `ty`) follows §14.4 (refactoring as architecture design)
— each file owns a grammar section. The driver's `lower_crate`
returns `(HirCrate, Vec<LowerError>)` so Stage 18.75 P0-A can surface
lowering errors into `CompileErrors.lower` instead of silently
discarding them.
