# Typeck Data Flow (MIR + tables → typed MIR)

> **Date**: 2026-08-31
> **Version**: v0.557.0
> **Stage**: 30.22 (graph docs gap closure)

## Module Overview

Typeck is the type inference + constraint-solving pass over MIR bodies.
Per 03-type-system.md, the type checker walks each `BasicBlock` in order,
inspects every `Statement::Assign(place, rvalue)` and each `Terminator`
(`Call`, `SwitchInt`, …), infers types for operands, and unifies them
against declared place types. After fixpoint, unresolved int/float
inference variables are defaulted to `i32` / `f64`, and remaining
unifications produce `TypeError`s.

Stage 18.60 closed the long-standing §16 violation: typeck no longer
re-lowers HIR to MIR internally. The driver pre-computes a
`FieldTyTable` (struct/enum field types) and `FnSigTable` (function
signatures), hands them to `TypeChecker::check_mir_body_with_tables`,
and typeck reads zero HIR. Stage 6.15 (TD-025) split typeck into 6
sub-modules (`check`, `infer`, `writeback`, `predicates`, `tables`,
`where_clause`); Stage 17.03 added the v0.5 `solver` for associated
type projection + trait obligations.

## Data Flow Diagram

```
MirBody (from mir::lower)  +  FieldTyTable  +  FnSigTable
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  TypeChecker::new() / with_unify()    src/typeck/checker.rs │
│                                                               │
│  unify: UnificationTable   (InferVar → Ty resolution)         │
│  errors: Vec<TypeError>                                       │
│  results: TypeckResults    (per-body table)                  │
│  fn_sigs: HashMap<DefId, Sig> (from FnSigTable)             │
└─────────────┬────────────────────────────────────────────────┘
              │ check_mir_body_with_tables(mir, field_tys, fn_sigs)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Walk basic blocks (src/typeck/check.rs)                    │
│                                                               │
│  for bb in mir.basic_blocks:                                  │
│    for stmt in bb.statements:                                 │
│      check_assign(place, rvalue):                            │
│        infer rvalue type (infer.rs)                          │
│        unify(rvalue_ty, place_decl_ty)                       │
│    check_terminator(term):                                    │
│      Call  → unify args with fn_sigs[def_id].inputs           │
│              unify dest with fn_sigs[def_id].output           │
│      SwitchInt → unify discr with i32/u32/bool              │
│      Drop → check that place type implements Drop            │
│                                                               │
│  predicates.rs → type classification (is_int, is_ref, etc.) │
│  where_clause.rs → bound checking on generics                 │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│  default_unresolved + writeback (src/typeck/)                │
│                                                               │
│  - Default unresolved IntVar → i32, FloatVar → f64           │
│  - writeback.rs → propagate resolved types back into MirBody  │
│    (LocalDecl.ty, Place projections, etc.)                    │
│  - Collect TypeErrors into Vec<TypeError>                    │
└─────────────┬────────────────────────────────────────────────┘
              │ TypeckResults + modified MirBody
              ▼
┌─────────────────────────────────────────────────────────────┐
│  TypeckResults (src/typeck/tables.rs)                       │
│                                                               │
│  per-body type table keyed by HirId / LocalId                 │
│  consumed by: borrowck (Copy detection),                       │
│               codegen (resolved Ty on every Place),           │
│               driver (FnSigTable for call sites)                │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
              → mir::drop_elaboration (needs resolved types)
              → borrowck::check_mir_body_with_dataflow
              → codegen::codegen_function
```

## Key Data Structures

- **`TypeChecker`** (`src/typeck/checker.rs`) — Holds `unify:
  UnificationTable`, `errors: Vec<TypeError>`, `results: TypeckResults`,
  `fn_sigs: HashMap<DefId, Sig>`. Constructed via `new()` (default) or
  `with_unify(unify)` (re-using MIR lower's infer vars).
- **`UnificationTable`** (`src/typeck/unify.rs`) — Union-find over
  `InferVar` keys mapping to `Ty`. Supports `unify(a, b)`,
  `new_int_var()`, `new_float_var()`, `probe(var)`,
  `default_unresolved()`.
- **`TypeckResults`** (`src/typeck/tables.rs`) — Per-body table
  consumed by borrowck + codegen: maps `HirId` / `LocalId` to resolved
  `Ty`. Populated during `check_mir_body`.
- **`FieldTyTable`** (`src/typeck/tables.rs`) — Pre-computed by the
  driver from HIR (struct/enum field types), keyed by `DefId`. The
  typeck-only HIR-replacement artifact that satisfies §16.
- **`FnSigTable`** (`src/typeck/tables.rs`) — Pre-computed
  `HashMap<DefId, Sig>` for all functions in the crate. Lets typeck
  check `Call` terminators without re-reading HIR.
- **`TypeError` / `TypeErrorKind`** (`src/typeck/error.rs`) —
  Structured type error with `expected: Option<Ty>`, `found:
  Option<Ty>`, `span`, `message`. Surfaced via
  `CompileErrors.typeck` (non-fatal — MIR is still produced).

## Dependencies

**Upstream inputs:**
- `MirBody` from `mir::lower` (with `UnificationTable` side-output
  for sharing infer vars with MIR lower).
- `FieldTyTable` + `FnSigTable` from the driver's pre-computation
  phase (built from HIR — satisfies §16).
- `&TraitResolver` for `is_copy_builtin` and trait-method lookup
  (optional — used by predicates + writeback).

**Downstream consumers:**
- `src/borrowck/mod.rs` — reads `TypeckResults` for Copy detection.
- `src/codegen/*` — reads resolved `Ty` on every `Place` /
  `LocalDecl`; `FnSigTable` for call return types.
- `src/driver/mod.rs` — collects `TypeError`s, runs iterative fixpoint
  for nested closures (re-typeck until fn_sigs stable or 4 passes).

## Stage Boundaries

Per §16, typeck consumes only `MirBody` + pre-computed tables — never
HIR. Stage 18.60 removed the deprecated `check_crate` and
`check_mir_body_with_hir` free functions that violated §16. The
canonical entry is `check_mir_body_with_tables`; the convenience
wrapper `check_mir_body` is used by tests. Typeck sits at pipeline
position 6 (after MIR lower, before drop elaboration 6.5 and
borrowck 7). The 6-way file split (Stage 6.15 TD-025) follows §13.4
J1-J6: each file owns one responsibility (constraint collection,
inference, writeback, predicates, tables, where-clause checking).
The Stage 17.03 `solver` sub-module is the v0.5 trait obligation
solver (Phase 1 declares data structures; Phase 2+ adds Evaluation /
Selection / Fulfillment for associated type projection).
