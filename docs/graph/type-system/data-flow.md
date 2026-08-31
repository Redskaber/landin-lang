# Type System Data Flow (Typeck + Borrowck)

> **Date**: 2026-08-29
> **Version**: v0.510.0 (Stage 18.381 — Phase 0 + Phase 3.7 REMOVED, writeback phases 10 → 8)

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
│    Stage 18.357: substitute(resolved, substs) in step 1      │
│    Stage 18.380: substitute(field_ty, substs) in step 2      │
│      (writeback_field_load_locals_with_table)                │
│    Phase 3.7 REMOVED (Stage 18.380) — no longer needed!      │
│                                                               │
│  Phase 4: Populate TypeckResults                             │
│                                                               │
│  Phase 5: Post-defaulting terminator check                   │
│    "expected function, found i32" (after defaulting)         │
│                                                               │
│  ─── 5-layer substitute chain (Stage 18.347-18.376) ───     │
│  Layer 1: Phase 0 pre-writeback (Stage 18.353)              │
│    writeback_type_propagation runs BEFORE Phase 1            │
│    Resolves Param in local_decls before typeck sees them    │
│    ** REMOVED Stage 18.381 — redundant after 18.380 **       │
│  Layer 2: Phase 3.5 table substitute (Stage 18.357)         │
│    substitute(resolved, substs) in writeback_field_types    │
│  Layer 3: Phase 3.7 post-table re-writeback (Stage 18.355)   │
│    Fixes Phase 3.5 regression                                │
│    ** REMOVED Stage 18.380 — redundant after step 2 fix **   │
│  Layer 4: resolve_place_type_with_table (Stage 18.358)      │
│    Recursive substitute in Projection field_ty resolution    │
│  Layer 5: compute_use_writeback_ty (Stage 18.361)            │
│    Recursive Projection base resolution for nested access   │
│                                                               │
│  ─── Aggregate field_tys substitute (Stage 18.376) ───      │
│  writeback_field_types_in_rvalue_with_table:                │
│    For AggregateKind::Adt(_, _, substs, field_tys):         │
│      if substs non-empty AND field_ty contains Param:        │
│        field_ty = substitute(field_ty, substs)              │
│  Resolves nested generic struct literals:                    │
│    Outer<Inner<T>> { inner: Inner { val: 42i64 } }           │
│    → field_tys[0] = Inner<Param(0)> → Inner<i64>             │
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

## Generic Substs Data Flow (Task 11 Phase 1 — Stages 16.50-16.52)

```
Parser (parse_generics, try_parse_generic_args, turbofish)
    │
    ▼
AST (Generics, GenericParam, GenericArg, PathSegment.args)
    │
    ▼
HIR (HirGenerics, HirPathSegment.args preserved)
    │
    ├─────────────────────────────────────────────────┐
    ▼                                                 ▼
┌────────────────────────────────────┐    ┌────────────────────────────────────┐
│  generics_of query (Stage 16.50)   │    │  lower_path_generic_args (16.51)   │
│                                    │    │                                    │
│  src/hir/generics.rs:              │    │  src/mir/lower/mod.rs:             │
│    build_generics_map(hir)         │    │    path → SubstsRef                │
│      → HashMap<DefId, ParamTy[]>   │    │                                    │
│    generics_of(def_id, hir)        │    │  Walks path.segments.last().args,  │
│      → Vec<ParamTy>                │    │  extracts Type args, lowers each   │
│                                    │    │  to MIR Ty via lower_ast_ty_to_    │
│  Walks HIR owners, extracts        │    │  mir_ty (minimal AST→MIR).         │
│  HirGenerics, filters type params  │    │                                    │
│  (skip lifetimes).                 │    │  Lifetime + Assoc args skipped.    │
└────────────────┬───────────────────┘    └────────────────┬───────────────────┘
                 │                                         │
                 │           ┌─────────────────────────────┘
                 │           │
                 │           ▼
                 │  ┌──────────────────────────────────────────────────┐
                 │  │  TyKind::Adt substs (Stage 16.51, Phase 1b)     │
                 │  │                                                  │
                 │  │  lower_hir_ty_to_mir_ty_with_regions:           │
                 │  │    Res::Def(def_id) →                            │
                 │  │      Ty::new(Adt(def_id, substs), span)          │
                 │  │                                                  │
                 │  │  Affects: type annotations, fn sigs, local decls│
                 │  └──────────────────┬───────────────────────────────┘
                 │                     │
                 │                     ▼
                 │  ┌──────────────────────────────────────────────────┐
                 │  │  AggregateKind::Adt substs (Stage 16.52, 1c)    │
                 │  │                                                  │
                 │  │  mir/lower/expr_operand.rs — 5 sites:           │
                 │  │    1. Enum unit variant path (Color::Red)        │
                 │  │    2. ADT ctor call (Pair(1, 2))                 │
                 │  │    3. Struct literal (Pair { a: 1, b: 2 })       │
                 │  │    4. Enum struct variant (Shape::Circle { r })  │
                 │  │    5. Fall-through ADT ctor path                 │
                 │  │                                                  │
                 │  │  All path-based sites use lower_path_generic_    │
                 │  │  args(path, &mut 0) (通解 — one helper).         │
                 │  │  Call-based site reuses adt_substs from          │
                 │  │  func_local_decl.ty.kind (DRY).                  │
                 │  └──────────────────┬───────────────────────────────┘
                 │                     │
                 ▼                     ▼
┌────────────────────────────────────────────────────────────────────┐
│  Typeck Unification (Stage 16.52)                                  │
│                                                                    │
│  typeck/unify.rs Adt arm:                                          │
│    LHS substs empty OR RHS substs empty → match by DefId only      │
│      (empty = "unknown, to be inferred")                           │
│    Both non-empty: must match length + unify element-wise          │
│                                                                    │
│  This is the principled rule (replaces Stage 16.51 relaxation).    │
│  Sound because empty substs = "no information".                    │
└────────────────────────────────────────────────────────────────────┘
```

### Generic Substs Status

| Phase | Status | Stage | Description |
|-------|--------|-------|-------------|
| 1a | ✅ | 16.50 | `generics_of` query |
| 1b | ✅ | 16.51 | Substs in `TyKind::Adt` (type annotations) |
| 1c | ✅ | 16.52 | Substs in `AggregateKind::Adt` (literal construction) |
| 2 | ✅ | 16.53 | `substitute(ty, substs)` + field type resolution |
| 3 | ✅ | 16.54 | `collect_mono_items` — walk MIR, dedup (def_id, substs) |
| 4a | ✅ | 16.55 | Specialized naming (`mangle_ty`, `mono_item_name`) |
| 4b-pre | ✅ | 16.56 | Nested generic args resolution (prerequisite for 4b) |
| 4b | ✅ | 16.57 | Per-mono layouts (`MonoLayoutKey`, `build_mono_layouts`) |
| 4c | ✅ | 16.58 | Codegen integration (`lookup_mono_layout`, `_and_mono`) |

### Per-Mono Codegen Naming Data Flow (Stage 16.55, Phase 4a)

```
MonoItem { def_id, substs }
    │
    ▼
┌────────────────────────────────────────────────────────────┐
│  build_mono_item_names(items, fn_names, type_names, interner)│
│                                                            │
│  For each MonoItem:                                        │
│    Type { def_id, .. } → base from type_name_by_def_id     │
│    Fn { def_id, .. } → base from fn_name_by_def_id         │
│                         (stripped of "landin_" prefix)     │
│    Closure { def_id, .. } → "closure_<def_id>"             │
│                                                            │
│  Then: mono_item_name(item, base_name, type_names, interner)│
│    → mangle_ty_with_interner(subst, type_names, interner)  │
│      for each subst                                        │
│    → "<base>_<mangled_subst1>_<mangled_subst2>..."         │
│                                                            │
│  Per §23: <verb>_<noun>_<noun>_<noun> pattern              │
└────────────────────────┬───────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────────────┐
│  HashMap<MonoItem, String> — specialized names             │
│                                                            │
│  Examples:                                                 │
│    Type { Box, [i32] }  → "Box_i32"                        │
│    Type { Box, [bool] } → "Box_bool"                       │
│    Fn { id, [i32] }     → "id_i32"                         │
│    Fn { id, [bool] }    → "id_bool"                        │
│    Closure { 3, [i32] } → "closure_3_i32"                  │
│                                                            │
│  Consumed by Phase 4b-4c (codegen integration):            │
│    Layouts keyed by (DefId, SubstsRef)                     │
│    Functions: landin_Box_i32, landin_Box_bool              │
└────────────────────────────────────────────────────────────┘
```

### Monomorphization Collection Data Flow (Stage 16.54, Phase 3)

```
MIR Bodies (Vec<MirBody>)
    │
    ▼
┌────────────────────────────────────────────────────────────┐
│  collect_mono_items(mirs) -> Vec<MonoItem>                  │
│                                                            │
│  Walks each MIR body:                                      │
│    1. local_decls[i].ty — local variable types             │
│    2. statements — Rvalue::Aggregate, Rvalue::Cast         │
│    3. terminators — Call { func, args }                    │
│    4. projection elements — Field(_, ty)                   │
│                                                            │
│  For each type, calls collect_from_ty:                     │
│    Adt(def_id, substs)   → MonoItem::Type (if non-empty)   │
│    FnDef(def_id, substs) → MonoItem::Fn   (if non-empty)   │
│    Closure(def_id, substs) → MonoItem::Closure (if non-    │
│                              empty)                        │
│    Recursively walks inner substs, Ref, Tuple, Array, etc. │
│                                                            │
│  Deduplicates via HashSet<MonoItem>                        │
│  Per §23: <verb>_<noun>_<noun> pattern                     │
│  Per §16: reads MIR only (no HIR access)                   │
└────────────────────────┬───────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────────────┐
│  Vec<MonoItem> — deduplicated specializations              │
│                                                            │
│  Each MonoItem = one concrete instantiation:               │
│    Type { def_id: Vec, substs: [i32] }   = Vec<i32>        │
│    Type { def_id: Vec, substs: [bool] }  = Vec<bool>       │
│    Fn { def_id: id, substs: [i32] }      = id::<i32>       │
│    Fn { def_id: id, substs: [bool] }     = id::<bool>      │
│                                                            │
│  Consumed by Phase 4 (per-mono codegen):                   │
│    Layouts keyed by (DefId, SubstsRef)                     │
│    Functions: landin_vec_push_i32, landin_vec_push_bool    │
└────────────────────────────────────────────────────────────┘
```

### Type Substitution Data Flow (Stage 16.53, Phase 2)

```
Generic struct field type (HIR)         Concrete field type (MIR)
  struct Box<T> { val: T }                b.val : i32
         │                                      ▲
         ▼                                      │
┌────────────────────────────────────┐   ┌──────┴──────────────────────────┐
│  lower_hir_ty_to_mir_ty_with_     │   │  substitute(field_ty, substs)   │
│  generics(ty, generic_params)     │   │                                  │
│                                   │   │  Param(0) + [i32] → i32         │
│  Resolves T → Param(ParamTy {     │   │  Adt(Box, [Param(0)]) + [i32]  │
│    index: 0, name: T })           │   │    → Adt(Box, [i32])            │
│                                   │   │                                  │
│  Per §23: <verb>_<noun>_<noun>    │   │  Per §23: <verb> (pure fn)      │
│  _<prep>_<noun> pattern           │   │  Per §16: reads Ty only          │
└────────────────┬───────────────────┘   └──────────────┬───────────────────┘
                 │                                       │
                 └───────────────────┬───────────────────┘
                                     │
                                     ▼
                ┌────────────────────────────────────────────┐
                │  resolve_adt_field_tys_with_substs         │
                │  resolve_field_type                        │
                │                                            │
                │  1. Get generic_params via generics_of     │
                │  2. Lower field type with generics          │
                │  3. Apply substitute(field_ty, substs)      │
                │  4. Return concrete field type              │
                │                                            │
                │  Per §23: <verb>_<noun>_<noun>_<noun>      │
                │  _<prep>_<noun> pattern                     │
                └────────────────────────────────────────────┘
```

---

## Lifetime Elision Data Flow (Stage 30.2, v0.13)

> **Date**: 2026-08-31
> **Version**: v0.543.0 (Stage 30.2 — TD-STUB-LIFETIME-ELISION-NOOP Rule 4 enforcement)

```
HIR Body (function with ref params + ref return)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  MIR Lowering — Elision Pass (mir/lower/body_lower.rs)        │
│                                                               │
│  for each param:                                              │
│    if self_kind.is_some():                                    │
│      resolve_self_param_type(...) → Region::Var(N)            │
│        (Stage 30.2: was Region::Erased, made rule 3 no-op)   │
│      collect self's vid → self_region_vid                     │
│      push to param_region_vids_collected                       │
│    elif param.ty.is_some():                                   │
│      lower_hir_ty_to_mir_ty_with_lifetimes(ty)                 │
│      collect_region_vids → param_region_vids_collected         │
│                                                               │
│  Then lower return type with lifetime_map:                    │
│    lower_hir_ty_to_mir_ty_with_lifetimes(return_ty)           │
│                                                               │
│  Rule 4 Check (Stage 30.2 — NEW):                              │
│    rule_applies = (n_inputs == 1)                             │
│               OR (n_inputs > 1 AND self_vid.is_some())        │
│    if !rule_applies:                                          │
│      if find_elided_ref_span(return_ty).is_some():            │
│        push TypeError("missing lifetime specifier")           │
│                                                               │
│  Apply Elision Rules 2/3 (with explicit_vids filter):          │
│    explicit_vids = lifetime_map.values().collect()             │
│    apply_elision_rules(return_ty, input_vids, self_vid,       │
│                       explicit_vids)                          │
│      → replaces only vids NOT in explicit_vids                │
│        (Stage 30.2: was unconditional replace — silent       │
│         over-application bug; now preserves explicit)        │
│                                                               │
│  Per §1.0 原則 4 (报错 > 静默): rule 4 enforced explicitly.  │
│  Per §1.0 原則 9 (正确 > 妥协): preserve explicit lifetimes. │
│  Per §12 (最优 > 最小): root-cause fix at lowering stage.    │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Region Inference (borrowck/region_inference.rs)              │
│                                                               │
│  infer_regions() — fixed-point iteration over MIR:            │
│    'a: 'b constraints from function signature                 │
│    lifetime relationships from borrows                        │
│    universal region checks (for<'a> support)                  │
│    type tests (region unification)                            │
│                                                               │
│  Now receives properly-elided regions (not Region::Erased     │
│  for &self), so rule 3 actually fires and produces correct    │
│  lifetime relationships.                                      │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
                     Typeck Errors
            (including "missing lifetime specifier"
             for rule 4 violations)
```

### Key Functions

| Function | Location | Purpose |
|----------|----------|---------|
| `find_elided_ref_span` | `mir::lower::body_lower` | Walk HIR type, return span of first `Ref(None, ...)` |
| `apply_elision_rules` | `mir::lower::body_lower` | Replace elided output refs with target_vid (preserving explicit) |
| `collect_region_vids` | `mir::lower::body_lower` | Gather all `Region::Var(N)` from a MIR type |
| `resolve_self_param_type` | `mir::lower::body_lower` | Build `&Self`/`&mut Self`/`Self` type for self param |

---

## HRTB (Higher-Ranked Trait Bounds) Data Flow (Stage 30.5, v0.13)

> **Date**: 2026-08-31
> **Version**: v0.546.0 (Stage 30.5 — TD-GAT-HIGHER-RANKED partial implementation)

```
Source: `fn bar<T: for<'a> Foo<'a>>(x: &T) { }`
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  Parser (src/parser/generics.rs)                               │
│                                                               │
│  parse_type_bounds() encounters `for` keyword:                │
│    1. bump `for`                                              │
│    2. parse_for_lifetime_params() → Vec<LifetimeParam>        │
│       - parse `'ident (, 'ident)*`                            │
│       - consume closing `>`                                    │
│    3. recursively parse inner bound (Trait path)              │
│    4. construct TypeBound::ForLifetimes {                     │
│         lifetime_params, bound, span                           │
│       }                                                        │
│                                                               │
│  Per §1.0 原則 3 (显式 > 隐式): HRTB is explicit in AST.     │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  AST (src/ast/kinds.rs)                                       │
│                                                               │
│  TypeBound::ForLifetimes {                                    │
│    lifetime_params: Vec<LifetimeParam>,                       │
│    bound: Box<TypeBound>,  // inner trait bound               │
│    span: Span,                                                │
│  }                                                            │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  HIR Lower (src/hir/lower/generics.rs)                        │
│                                                               │
│  lower_type_bound() ForLifetimes arm:                         │
│    - Convert AST LifetimeParam → HirLifetimeParam             │
│    - Recursively lower inner bound                            │
│    - Construct HirTypeBound::ForLifetimes { ... }             │
│                                                               │
│  Per §1.0 原則 9 (正确 > 妥协): surface syntax lowered;      │
│  solver integration deferred to v0.14+.                       │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  HIR (src/hir/kinds.rs)                                       │
│                                                               │
│  HirTypeBound::ForLifetimes {                                 │
│    lifetime_params: Vec<HirLifetimeParam>,                    │
│    bound: Box<HirTypeBound>,                                  │
│    span: Span,                                                │
│  }                                                            │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Trait Solver (src/traits/solver/) — NOT YET WIRED            │
│                                                               │
│  Binder<T> infrastructure EXISTS (mod.rs:116-150) but is     │
│  not used for HRTB. The bound is treated as a regular        │
│  trait bound during selection.                                │
│                                                               │
│  TD-HRTB-SOLVER-INTEGRATION (v0.14+) will wire:              │
│    1. Binder<T> into trait selection                          │
│    2. enter_universe/restore_universe for placeholder        │
│       regions                                                 │
│    3. Verify bound holds for ALL 'a (not just one)            │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
                Compiles cleanly
            (HRTB captured but not enforced —
             TD-HRTB-SOLVER-INTEGRATION needed
             for full soundness)
```

### Key Components

| Component | File | Status |
|-----------|------|--------|
| Parser: `parse_type_bounds` + `parse_for_lifetime_params` | `src/parser/generics.rs` | ✅ Stage 30.5 |
| AST: `TypeBound::ForLifetimes` | `src/ast/kinds.rs` | ✅ Stage 30.5 |
| HIR: `HirTypeBound::ForLifetimes` | `src/hir/kinds.rs` | ✅ Stage 30.5 |
| HIR Lower: `lower_type_bound` | `src/hir/lower/generics.rs` | ✅ Stage 30.5 |
| `Binder<T>` infrastructure | `src/traits/solver/mod.rs:116` | ✅ Exists (not wired) |
| `enter_universe`/`restore_universe` | `src/borrowck/region_inference.rs` | ✅ Exists (not wired) |
| Trait solver HRTB enforcement | `src/traits/solver/` | ❌ TD-HRTB-SOLVER-INTEGRATION (v0.14+) |
| `Fn(...)` trait call syntax | `src/parser/` | ❌ TD-HRTB-FN-SYNTAX (v0.14+) |
