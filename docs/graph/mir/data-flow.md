# MIR Data Flow (HIR → MIR CFG)

> **Date**: 2026-08-31
> **Version**: v0.557.0
> **Stage**: 30.22 (graph docs gap closure)

## Module Overview

MIR (Mid-level Intermediate Representation) is the control-flow-graph
form of each function body. Per 06-mir.md it is the input to type
inference (Stage 2.2 unification), borrow check (Stage 2.3 NLL), and
LLVM codegen (Stage 3). A `MirBody` is a vector of basic blocks, each
containing a sequence of `Statement`s (`Place = Rvalue`) followed by
a single `Terminator` (Goto / SwitchInt / Call / Return / Unreachable).

The lowering pass (`src/mir/lower/`) walks HIR bodies in pre-order,
allocates `LocalId`s for params and lets, builds `Place` / `Rvalue` /
`Operand` nodes, and emits `BasicBlock`s. Stage 3.47 closed L-PIPE-1:
MIR lower sinks `AdtLayouts` (an `Arc<HashMap<DefId, AdtLayout>>`)
into `MirBody` so codegen never has to read HIR. Stage 16.54 added
monomorphization collection (`collect_mono_items`) so codegen can
emit specialized per-type functions.

## Data Flow Diagram

```
hir::Body (one per fn/closure/const)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  MirLowerCtxt                          src/mir/lower/mod.rs │
│                                                               │
│  body: HirBody, hir: &HirCrate, interner: &Rodeo            │
│  next_local: u32 (LocalId counter)                            │
│  next_bb: u32 (BasicBlockId counter)                          │
│  unify: UnificationTable (fresh InferVar for literals)        │
│  shared_unify: Option<&mut UnificationTable>                 │
│  closure_def_id_counter, errors, nested_closures              │
└─────────────┬────────────────────────────────────────────────┘
              │ lower_hir_body_to_mir_full(...)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Lowering pipeline (16 sibling sub-modules)                  │
│                                                               │
│  body_lower.rs       → block, stmt, let, return              │
│  expr_operand.rs    → lower_expr_to_operand (entry)         │
│  expr_variants.rs    → 28 HirExprKind variants                │
│  call_lower.rs       → TerminatorKind::Call construction    │
│  method_call_lower.rs → receiver.method(args) (634 LOC)     │
│  method_resolution.rs → query_method_return_type (cached)    │
│  field_resolution.rs → struct/tuple field projection         │
│  control_flow.rs     → if / while / for / loop / match / break│
│  pattern_lower.rs    → match arms, struct/tuple destructure   │
│  pattern_bindings.rs → bindings (let, fn param, match arm)   │
│  ty_lower.rs         → HIR Ty → MIR Ty (Param/Infer/Adt/...)  │
│  adt_layout.rs       → AdtLayout (Struct/Enum) sink          │
│  closure_capture.rs  → closure captures (Stage 16.35)        │
│  writeback.rs        → tuple literal / field writeback        │
│  overflow_assert.rs  → debug_assert! on arithmetic          │
│  primitive_intrinsics / string_intrinsics / vec_intrinsics /  │
│  box_intrinsics / format_intrinsics → intrinsic dispatch     │
└─────────────┬────────────────────────────────────────────────┘
              │ MirBody
              ▼
┌─────────────────────────────────────────────────────────────┐
│  MirBody (src/mir/body.rs)                                   │
│                                                               │
│  basic_blocks: Vec<BasicBlock>                                │
│  local_decls: Vec<LocalDecl>  (params + locals + temps)     │
│  span: Span                                                   │
│  adt_layouts: SharedAdtLayouts (Arc<HashMap<DefId, AdtLayout>>)│
│  source: Option<DefId>  (owner fn / closure)                 │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
              → typeck::check_mir_body_with_tables (writes types)
              → mir::drop_elaboration (insert Drop terminators)
              → borrowck::check_mir_body_with_dataflow (NLL)
              → codegen::codegen_function (emit LLVM IR)
```

## Key Data Structures

- **`MirBody`** (`src/mir/body.rs`) — The CFG of one function:
  `basic_blocks: Vec<BasicBlock>`, `local_decls: Vec<LocalDecl>`,
  `span`, `adt_layouts: SharedAdtLayouts`. Carries sunk HIR-derived
  data so codegen needs no HIR access (§16).
- **`Place` / `PlaceKind` / `ProjectionElem`** (`src/mir/place.rs`)
  — Addressable locations: `Local(LocalId) | Static(DefId) |
  Projection(Box<Place>, ProjectionElem)` where `ProjectionElem` is
  `Deref | Field(FieldId, Ty) | Index(LocalId) | ConstantIndex |
  Subslice`. Renamed from `Lvalue` in Stage 3.66.
- **`Rvalue` / `Operand`** (`src/mir/place.rs`) — RHS of `Place =
  Rvalue`. Rvalue variants: `Use`, `Repeat`, `Ref`, `BinaryOp`,
  `UnaryOp`, `Cast`, `Aggregate`, `Len`, `Discriminant`, … Operand
  variants: `Copy(Place) | Move(Place) | Constant(Const)`.
- **`BasicBlock` / `Terminator` / `TerminatorKind`** (`src/mir/body.rs`)
  — `BasicBlock { statements: Vec<Statement>, terminator: Terminator }`.
  TerminatorKind: `Goto | SwitchInt | Call | Return | Unreachable |
  Drop | Yield`. Local 0 is the return local; Local 1 is closure self.
- **`Ty` / `TyKind` / `Sig`** (`src/mir/ty.rs`) — MIR-side type
  representation including `Infer(InferVar)`, `Param(u32)`,
  `Adt(DefId, SubstsRef)`, `Closure(DefId, SubstsRef)`, etc.
- **`AdtLayout` / `AdtLayouts`** (`src/mir/body.rs`) — Struct / Enum
  storage layout sunk from HIR. `Enum { discriminant_ty,
  variant_payloads: Vec<Vec<Ty>> }` is forward-compatible for the
  Stage 4 L-ENUM-UNION fix.

## Dependencies

**Upstream inputs:**
- `hir::Body` (one per fn/closure/const), `&HirCrate` (for field
  lookups, generics, impl metadata), `&Rodeo` interner.
- `Option<&mut UnificationTable>` from the driver for shared infer
  vars across bodies.

**Downstream consumers:**
- `src/typeck/checker.rs::check_mir_body_with_tables` — walks blocks,
  collects type constraints, unifies.
- `src/borrowck/mod.rs::check_mir_body_with_dataflow` — NLL liveness +
  move tracking + borrow set.
- `src/mir/drop_elaboration.rs` — inserts `Drop` terminators where
  owned values go out of scope (Stage 15.43).
- `src/mir/monomorphize/mod.rs` — collects `MonoItem { def_id, substs }`
  for codegen (Stage 16.54).
- `src/mir/optimization.rs` — DCE + const propagation (Stage 17.10).
- `src/codegen/function.rs` — emits LLVM IR per `MirBody`.

## Stage Boundaries

Per §16, MIR lower reads HIR (allowed — data flows downstream), but
typeck/borrowck/codegen never read HIR directly: they consume
pre-computed tables (`FieldTyTable`, `FnSigTable`,
`SharedAdtLayouts`) that the driver builds from HIR and hands to MIR.
The MIR lower is at pipeline position 5 (after resolve, before
typeck). The 16-file lowering split follows §13.4 J1-J6 (single
responsibility) — each file owns a lowering concern. The closure
data flow (see `closure/data-flow.md`) is a sub-pipeline inside MIR
lower that emits a `SynthesizedClosureFunction` per closure. The
`param_check.rs` (Stage 18.348) is a pre-codegen diagnostic pass
that reports unresolved `Param`/`Infer`/`Error`/`Projection` types
so codegen doesn't silently map them to `EmitType::I32`.
