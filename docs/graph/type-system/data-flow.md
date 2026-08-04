# Type System Data Flow (Typeck + Borrowck)

> **Date**: 2026-08-04
> **Version**: v0.235.1

## Type Checking Data Flow

```
HIR Body
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  MIR Lowering (mir/lower/)                                   │
│                                                               │
│  lower_hir_body_to_mir_full_with_dyn_trait_plan(body, hir)  │
│    → (MirBody, UnificationTable, TypeErrors,                │
│       SynthesizedClosureFunctions)                           │
│                                                               │
│  Fresh Infer vars created during lowering:                   │
│    TyVar(TyVid(N))  — general type variable                  │
│    IntVar(IntVid(N)) — integer type variable                 │
│    FloatVar(FloatVid(N)) — float type variable               │
│                                                               │
│  Shared unify table (Stage 16.29):                           │
│    Main body + all closure MIR bodies share one table        │
│    Prevents TyVid collision (root cause of stack overflow)   │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Type Checking (typeck/checker.rs)                           │
│                                                               │
│  TypeChecker::check_mir_body_with_tables(mir, field_ty)     │
│                                                               │
│  Phase 1: Walk basic blocks, collect constraints             │
│    For each Assign(place, rvalue):                           │
│      place_ty = infer_place(mir, place)                      │
│      rvalue_ty = infer_rvalue(mir, rvalue, span)             │
│      unify(place_ty, rvalue_ty)                               │
│                                                               │
│    For each Call(func, args, dest):                          │
│      If FnDef(def_id): look up sig in fn_sigs                │
│      If Closure(def_id): look up sig (skip self)             │
│      unify each arg with sig.inputs                          │
│      unify dest with sig.output                              │
│                                                               │
│  Phase 2: default_unresolved()                               │
│    IntVar → I32, FloatVar → F64                              │
│                                                               │
│  Phase 3: Writeback to local_decls                           │
│    local.ty = unify.resolve(&local.ty)                       │
│                                                               │
│  Phase 3.5: Writeback field types (via FieldTyTable)         │
│                                                               │
│  Phase 4: Populate TypeckResults                             │
│                                                               │
│  Phase 5: Post-defaulting terminator check                   │
│    "expected function, found i32" (after defaulting)         │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Iterative Typeck (Stage 16.32)                              │
│                                                               │
│  For closures (has_closures = true):                         │
│    Pass 1: Typeck all closures + main body                   │
│    Pass 2+: clear_bindings + re-typeck                       │
│      (capture types now resolved → inner closures resolve)   │
│    Stop at fixpoint (fn_sigs unchanged) or max 4 passes      │
│                                                               │
│  Closure-typed func in check_terminator (Stage 16.32):       │
│    If TyKind::Closure(def_id, _):                             │
│      look up sig, skip self, unify args, unify dest          │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Borrow Checking (borrowck/mod.rs)                           │
│                                                               │
│  BorrowChecker::check_mir_body_with_dataflow(mir)            │
│                                                               │
│  Sound Copy detection (Stage 16.06):                         │
│    ty_is_copy_with_resolver(ty, resolver, interner)          │
│    Closure Copy derivation (Stage 16.29):                    │
│      Closure(_, substs) → all substs Copy? → Closure is Copy │
│                                                               │
│  NLL fixpoint liveness:                                      │
│    compute_last_use_map — kill decision (borrow lifetimes)   │
│    compute_ever_read — GAP-1 preservation                    │
│    kill_borrows_on_redefinition — loop borrow temps          │
│                                                               │
│  Region inference (borrowck/region_inference.rs):            │
│    RegionInferenceContext — SCC-based region constraints     │
│    Universal regions, implied bounds, type tests             │
│                                                               │
│  Closure MIR body borrowck (Stage 16.31):                    │
│    Capture mutability propagated to extract locals           │
│    Allows `x += 1` where x is captured `mut`                 │
└─────────────────────────────────────────────────────────────┘
```

## Key Data Structures

```
UnificationTable (typeck/unify.rs):
  ty_vars: Vec<Option<Ty>>      — general type variable bindings
  int_vars: Vec<IntVarBinding>  — integer variable bindings (union-find)
  float_vars: Vec<FloatVarBinding> — float variable bindings
  errors: Vec<TypeError>        — unification errors

FieldTyTable (typeck/tables.rs):
  Maps Adt DefId → Vec<Ty> (field types)
  Built by driver from HIR before typeck

FnSigTable (typeck/tables.rs):
  Maps DefId → Sig { inputs, output, abi, is_unsafe }
  Populated by driver for all functions + closures
```

## MIR → EmitType Translation

```
mir_type_to_emit_type_with_layouts(ty, layouts) → EmitType

TyKind::Int(I32)          → EmitType::I32
TyKind::Ref(_, _, inner)  → EmitType::Ptr(Box<inner>)
TyKind::Tuple(tys)        → EmitType::Struct(tys)
TyKind::Adt(def_id, _)    → EmitType::Struct(field_tys) via AdtLayouts
TyKind::Closure(_, substs)→ EmitType::Struct(substs)
TyKind::FnDef(_, _)       → EmitType::OpaquePtr
```
