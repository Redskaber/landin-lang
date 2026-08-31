# Traits Data Flow (HIR → TraitResolver + Solver)

> **Date**: 2026-08-31
> **Version**: v0.557.0
> **Stage**: 30.22 (graph docs gap closure)

## Module Overview

The traits module collects trait definitions, impl blocks, and
associated types from HIR, then builds dispatch tables and answers
trait queries for typeck / borrowck / codegen. Per §16,
`TraitResolver` runs in the driver's pre-computation phase (reads HIR
once), then provides a data-only contract to every downstream pass —
no later pass reads HIR for trait metadata.

Stage 5.23 split the module into 3 sub-files (`vtable.rs`,
`builtin.rs`, `resolver.rs`). Stage 18.308 P3 extracted
`resolver_queries.rs` (J1-J6 single-responsibility extraction). Stage
16.64 added `object_safety.rs` (object safety checking for `dyn
Trait`). Stage 19.1 (v0.5 Phase 1) added the `solver/` sub-tree —
TraitPredicate, Goal, InferCtxt, ObligationQueue, EvalResult,
SelectionResult — for v0.5 associated type projection. Stage 30.10
(v0.14) added HRTB bound collection (`HrtbBound`), and Stage 30.12
(v0.15) added `assoc_type_bindings` for `Self::Item` resolution.

## Data Flow Diagram

```
HirCrate (from hir::lower)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  TraitResolver::new()            src/traits/resolver.rs     │
│                                                               │
│  trait_impls: HashMap<DefId, Vec<DefId>> (trait → impls)     │
│  impls_by_def_ids: HashMap<(DefId, DefId), DefId>            │
│  vtables_by_def_ids: HashMap<(DefId, DefId), Vtable>        │
│  derived_copy_types: HashSet<DefId>  (field-level Copy)      │
│  derived_clone_types, derived_drop_types                     │
│  traits: HashMap<DefId, TraitInfo>                           │
│  impls: HashMap<DefId, ImplInfo>                             │
└─────────────┬────────────────────────────────────────────────┘
              │ collect(&hir, &interner)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Walk HirCrate.owners (src/traits/resolver.rs)              │
│                                                               │
│  for owner in hir.owners:                                      │
│    match owner {                                               │
│      HirItem::Trait(t) → register TraitInfo                    │
│        { def_id, name, methods, supertraits,                  │
│          default_methods, associated_consts, is_unsafe }       │
│      HirItem::Impl(i)  → register ImplInfo                     │
│        { def_id, trait_name?, self_ty_name, methods,          │
│          is_unsafe, span, associated_consts,                  │
│          where_clauses, hrtb_bounds, assoc_type_bindings }    │
│      HirItem::Struct/Enum → check field-level Copy            │
│        derived_copy_types.insert(def_id) if all fields Copy   │
│      _ → skip                                                   │
│    }                                                            │
│  post: populate_def_id_keyed_maps()                           │
│    (impls_by_def_ids, vtables_by_def_ids — Task 3)            │
│                                                               │
│  Built-in dispatch (src/traits/builtin.rs):                   │
│    BUILTIN_TRAIT_NAMES, BUILTIN_PRIMITIVE_COPY_KINDS          │
│    is_primitive_copy_kind(interner, name)                     │
│                                                               │
│  Object safety (src/traits/object_safety.rs):                │
│    check_trait_object_safety(trait_info) → Vec<violation>     │
└─────────────┬────────────────────────────────────────────────┘
              │ &TraitResolver
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Query API (src/traits/resolver_queries.rs)                 │
│                                                               │
│  is_copy_builtin(def_id, interner) → bool                    │
│  is_clone_builtin(def_id, interner) → bool                   │
│  is_drop_builtin(def_id, interner) → bool                    │
│  find_impl_by_def_ids(trait_def_id, type_def_id) → Option<DefId>│
│  implements_by_def_ids(trait_def_id, type_def_id) → bool     │
│  find_vtable_by_def_ids(trait_def_id, type_def_id) → Option<Vtable>│
│  impl_methods_by_def_ids(trait_def_id, type_def_id) → Vec<method>│
└─────────────┬────────────────────────────────────────────────┘
              │ consumed by:
              ▼
              → typeck (Copy detection, trait-method sig lookup)
              → borrowck (sound ty_is_copy_with_resolver)
              → mir::lower (method_resolution query_method_return_type)
              → codegen (vtable layout for dyn Trait)
              → driver (pre-builds FieldTyTable/FnSigTable using resolver)
```

## Key Data Structures

- **`TraitResolver`** (`src/traits/resolver.rs`) — Holds all
  collected trait/impl/copy/drop/clone metadata + DefId-keyed lookup
  maps. Constructed once per compilation via `collect(&hir, &interner)`.
- **`TraitInfo`** (`src/traits/resolver.rs`) — Per-trait metadata:
  `{ def_id, name, methods, is_unsafe, supertraits, default_methods,
  associated_consts }`. Used by object safety + impl completeness.
- **`ImplInfo`** (`src/traits/resolver.rs`) — Per-impl metadata:
  `{ def_id, trait_name?, self_ty_name, methods, is_unsafe, span,
  associated_consts, where_clauses, hrtb_bounds, assoc_type_bindings }`.
  Stage 30.10 added `hrtb_bounds: Vec<HrtbBound>`; Stage 30.12 added
  `assoc_type_bindings: HashMap<Spur, Ty>`.
- **`Vtable` / `VtableEntry`** (`src/traits/vtable.rs`) — Static
  dispatch table: `Vtable { trait_def_id, type_def_id, entries:
  Vec<VtableEntry> }`. `VtableEntry { method_name, method_def_id }`.
  Slot index = method declaration order in trait.
- **`HrtbBound`** (`src/traits/resolver.rs`) — `for<'a, 'b> Trait`
  bound: `{ bounded_type_name, trait_def_id, lifetime_param_count }`.
  Collected in Stage 30.10; full enforcement deferred (TD-HRTB-FULL).
- **`TraitPredicate` / `Goal` / `InferCtxt` / `ObligationQueue`**
  (`src/traits/solver/`) — v0.5 trait obligation solver data
  structures (Phase 1 declares; Phase 2+ adds Evaluation /
  Selection / Fulfillment).

## Dependencies

**Upstream inputs:**
- `&HirCrate` for trait/impl/struct/enum collection.
- `&Rodeo` for symbol lookup (trait/impl/method names).

**Downstream consumers:**
- `src/typeck/checker.rs` — Copy detection, trait-method sig lookup
  via `find_impl_by_def_ids` and `impl_methods_by_def_ids`.
- `src/borrowck/mod.rs::with_resolver` — sound Copy detection
  (Stage 14.106 HP-1 fix; replaces unsound `ty_is_copy`).
- `src/mir/lower/method_resolution.rs` — `query_method_return_type`
  for resolving method call return types in MIR.
- `src/codegen/trait_dispatch/` — vtable layout for `dyn Trait` +
  static dispatch fn name resolution.
- `src/driver/mod.rs` — pre-computes TraitResolver, hands `&resolver`
  to typeck/borrowck/codegen.

## Stage Boundaries

Per §16, `TraitResolver` is the single point where HIR is read for
trait metadata. Every downstream pass receives `&TraitResolver` (a
data-only contract) — never reads HIR directly. The traits module
sits at pipeline position 4.5 (after resolve, parallel to MIR lower)
— the driver calls `collect(&hir)` before any MIR lower / typeck /
borrowck / codegen. The 5-file split (`builtin`, `error`,
`object_safety`, `resolver`, `resolver_queries`, `vtable`) follows
§13.4 J1-J6. The Stage 19.1 `solver/` sub-tree is a forward-looking
addition: v0.5 Phase 1 declares TraitPredicate / Goal / InferCtxt /
ObligationQueue; Phase 2+ adds Evaluation (`eval.rs`), Selection
(`select.rs`), Fulfillment (`fulfill.rs`), Supertrait resolution
(`supertrait.rs`). HRTB collection (Stage 30.10) is honest scope —
data is collected, full enforcement deferred to TD-HRTB-FULL.
