# Borrowck Data Flow (MIR + NLL → borrow-checked MIR)

> **Date**: 2026-08-31
> **Version**: v0.557.0
> **Stage**: 30.22 (graph docs gap closure)

## Module Overview

The borrow checker enforces Landin's ownership and borrowing rules on
MIR bodies. Per 04-ownership-borrowing.md it implements Non-Lexical
Lifetimes (NLL): lifetimes end at last use rather than at lexical scope
end. The rules are: (1) each value has a single owner; (2) `&T` allows
shared reads; (3) `&mut T` allows exclusive writes; (4) a value can
have multiple `&T` OR one `&mut T`, never both; (5) moves transfer
ownership; (6) a moved value cannot be used.

Stage 6.14 (TD-024) split borrowck into 3 sub-modules: `liveness.rs`
(NLL liveness analysis), `copy_semantics.rs` (Copy trait detection
for sound moves/borrows), `place_path.rs` (PlacePath data structure
used to track borrows + moves). Stage 7.1 (TD-015) added region
inference infrastructure (SCC, universes, type tests) — Stage 30.1
documented that `infer_regions()` IS called and works for `Region::Var`
cases; full HRTB integration is deferred to v0.13+. Stage 15.67
migrated to true Rust NLL semantics (liveness-based kill exclusively).

## Data Flow Diagram

```
MirBody (post-typeck) + TraitResolver (for Copy detection)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  BorrowChecker::with_resolver(...)   src/borrowck/mod.rs    │
│                                                               │
│  borrows: BorrowSet        (all active borrows)              │
│  moves: MoveTracker         (which locals moved)               │
│  errors: Vec<BorrowError>                                     │
│  initialized: HashSet<LocalId> (assigned at least once)      │
│  resolver: Option<&TraitResolver>  (sound Copy detection)    │
│  interner: Option<&Rodeo>                                     │
│  fn_sigs: Option<&HashMap<DefId, Sig>>  (region constraints)  │
└─────────────┬────────────────────────────────────────────────┘
              │ check_mir_body_with_dataflow(&mir)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Walk basic blocks (src/borrowck/mod.rs)                    │
│                                                               │
│  for bb in mir.basic_blocks:                                  │
│    for stmt in bb.statements:                                 │
│      match stmt.kind:                                         │
│        Assign(place, rvalue) →                                │
│          check_initialized (reject `x = 2` if x not let)     │
│          record Borrow on Rvalue::Ref                         │
│          record Move on Operand::Move                         │
│          check double-borrow / use-after-move                 │
│    match terminator.kind:                                     │
│      Call → mark args as moved (if non-Copy)                   │
│      Drop → check still-owned                                 │
│                                                               │
│  Sub-modules:                                                 │
│    liveness.rs       → compute_liveness (LiveIn/LiveOut maps) │
│    place_path.rs     → PlacePath, PlaceRoot, ProjElem         │
│    copy_semantics.rs → ty_is_copy_with_resolver               │
│    borrow_set.rs     → Borrow { assigned_place,               │
│                                borrowed_place, region, kind } │
│    move_tracker.rs   → MoveTracker (moved-set per block)      │
│    region_inference.rs → SCC + Universe + outlives constraints│
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│  NLL fixpoint + kill (Stage 15.67-15.71)                    │
│                                                               │
│  - Liveness-based kill: borrow dies at last use of its place  │
│  - compute_live_after_point(stmt) → which borrows are live    │
│  - kill_expired_borrows_dataflow()                           │
│  - region_inference.infer_regions() → 'static / Region::Var  │
│    outlives checks (called when fn_sigs present)              │
└─────────────┬────────────────────────────────────────────────┘
              │ Vec<BorrowError>
              ▼
              → CompileErrors.borrowck (non-fatal)
              → codegen (MirBody unchanged; borrowck is a check-only pass)
```

## Key Data Structures

- **`BorrowChecker<'a>`** (`src/borrowck/mod.rs`) — Holds `borrows`,
  `moves`, `errors`, `initialized`, optional `resolver` + `interner`
  for sound Copy detection (Stage 14.106 HP-1), optional `fn_sigs`
  for region inference constraints (Stage 15.71).
- **`BorrowSet` / `Borrow`** (`src/borrowck/borrow_set.rs`) —
  `Borrow { assigned_place: PlacePath, borrowed_place: PlacePath,
  region: Region, kind: BorrowKind }`. Indexable by `BorrowId`.
- **`MoveTracker`** (`src/borrowck/move_tracker.rs`) — Per-block set
  of moved locals. Used to reject use-after-move.
- **`PlacePath` / `PlaceRoot` / `ProjElem`** (`src/borrowck/place_path.rs`)
  — Borrowck-side place representation mirroring MIR `Place` but with
  explicit roots (`Local(LocalId) | Static(DefId)`) and projection
  elements (`Deref | Field | Index`).
- **`LiveInMap` / `LiveOutMap`** (`src/borrowck/liveness.rs`) —
  `HashMap<BasicBlockId, HashSet<LocalId>>` for the NLL liveness
  fixpoint. Stage 15.35 HP-10 re-exported the fixpoint liveness API.
- **`BorrowError` / `BorrowErrorKind`** (`src/borrowck/error.rs`)
  — Structured error: `UseAfterMove`, `DoubleMutBorrow`,
  `UninitializedAssign`, `MoveOutOfRef`, region outlives failures.

## Dependencies

**Upstream inputs:**
- `MirBody` from typeck (types resolved; infer vars defaulted).
- `&TraitResolver` (Stage 14.106 HP-1) for sound Copy detection —
  the unsound `ty_is_copy` (returns true for all Adt) was replaced.
- `&HashMap<DefId, Sig>` (Stage 15.71) for region inference
  constraints between call argument regions and parameter regions.

**Downstream consumers:**
- `src/driver/mod.rs` — collects `BorrowError` into
  `CompileErrors.borrowck` (non-fatal — MIR is still produced, but
  codegen may emit unsound code; user-visible errors surface to terminal).
- Borrowck does NOT mutate `MirBody` — it is a check-only pass.

## Stage Boundaries

Per §16, borrowck consumes MIR + resolver data; never HIR. The
canonical entry is `check_mir_body_with_dataflow` (Stage 15.40 driver
switch). Stage 6.14 TD-024 split follows §14.4: `liveness.rs` /
`copy_semantics.rs` / `place_path.rs` are sibling modules, each owning
one concern. Stage 7.1 TD-015 added `region_inference.rs`
infrastructure (SCC + Universe + type tests) — Stage 30.1 documented
that `infer_regions()` runs and catches real region errors when
`Region::Var` is present; full HRTB integration is v0.13+ (TD-GAT-
HIGHER-RANKED, TD-STUB-REGION-ERASED). Borrowck sits at pipeline
position 7 (after typeck 6, drop elaboration 6.5, before codegen 8).
The closure data flow (see `closure/data-flow.md`) interacts with
borrowck because closure capture extract locals carry mutability from
the outer scope, and `x += 1` where `x` is a captured `mut` is allowed.
