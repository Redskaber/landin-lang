# Trait System Data Flow (Static + Dynamic Dispatch)

> **Date**: 2026-08-04
> **Version**: v0.235.1

## Trait System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    TraitResolver (traits/resolver.rs)            │
│                                                                  │
│  Collected from HIR:                                             │
│  ├── trait_impls: HashMap<DefId, Vec<DefId>> — trait → impls    │
│  ├── impls_by_def_ids: HashMap<(DefId, DefId), DefId>           │
│  │   (trait_def_id, type_def_id) → impl_def_id                   │
│  ├── vtables_by_def_ids: HashMap<(DefId, DefId), Vtable>        │
│  ├── derived_copy_types: HashSet<DefId> — field-level Copy      │
│  └── populate_def_id_keyed_maps() — post-processing             │
│                                                                  │
│  Query methods:                                                  │
│  ├── is_copy_builtin(def_id, interner) → bool                   │
│  ├── is_clone_builtin(def_id, interner) → bool                  │
│  ├── is_drop_builtin(def_id, interner) → bool                   │
│  ├── find_impl_by_def_ids(trait, type) → Option<DefId>          │
│  ├── implements_by_def_ids(trait, type) → bool                  │
│  ├── find_vtable_by_def_ids(trait, type) → Option<Vtable>       │
│  └── impl_methods_by_def_ids(trait, type) → Vec<method>         │
└─────────────────────────────────────────────────────────────────┘
```

## Static Dispatch (Direct Call)

```
HIR: receiver.method(args)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  MIR Lowering                                                 │
│                                                               │
│  1. Resolve method: look up impl in TraitResolver            │
│     find_impl_by_def_ids(trait_def_id, type_def_id)          │
│                                                               │
│  2. Emit TerminatorKind::Call:                                │
│     func: Operand::Constant(FnDef(impl_method_def_id))        │
│     args: [receiver, args...]                                 │
│     destination: fresh local                                  │
│                                                               │
│  3. Typeck: look up sig in fn_sigs[impl_method_def_id]       │
│     Unify args with sig.inputs                                │
│     Unify dest with sig.output                                │
│                                                               │
│  4. Codegen: resolve fn_name via fn_name_by_def_id            │
│     emitter.emit_call(fn_name, args, ret_ty)                  │
└─────────────────────────────────────────────────────────────┘
```

## Dynamic Dispatch (dyn Trait)

```
HIR: let d: dyn Foo = &S; d.bar()
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  DynTraitMIRPlan (built by driver from TraitResolver)        │
│                                                               │
│  DynTraitFatPtr { trait_name, type_name, data_symbol,        │
│                   vtable_symbol, dynptr_symbol }              │
│                                                               │
│  DynTraitMethodCall { call_id, method_name, slot_index,      │
│                       dynptr_symbol, args }                    │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Codegen Pipeline (run_codegen_pipeline)                     │
│                                                               │
│  Step 2: emit_vtables                                         │
│    For each (trait, type) pair:                               │
│      emitter.emit_vtable_global(".vtable.Foo.S",              │
│                                 method_symbols)               │
│    → @.vtable.Foo.S = [ptr @landin_S_bar, ...]               │
│                                                               │
│  Step 3: emit_dyn_trait_ptrs                                  │
│    For each (trait, type) pair:                               │
│      emitter.emit_dyn_trait_const(".dynptr.Foo.S",            │
│                                    data_symbol,               │
│                                    vtable_symbol)             │
│    → @.dynptr.Foo.S = { ptr @.data.S, ptr @.vtable.Foo.S }  │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Terminator Codegen (terminator.rs)                           │
│                                                               │
│  TerminatorKind::Call { dyn_trait_call: Some(info), .. }:     │
│    1. Load vtable ptr from dynptr global (field 1)            │
│       %vtable = getelementptr @.dynptr.Foo.S, 0, 1           │
│       %vtable = load ptr, %vtable_ptr                         │
│                                                               │
│  2. Load method fn ptr from vtable at slot_index              │
│       %fn_ptr = getelementptr %vtable, 0, slot_index         │
│       %fn_ptr = load ptr, %fn_ptr                             │
│                                                               │
│  3. Indirect call                                             │
│       %result = call ret_ty %fn_ptr(args...)                  │
│                                                               │
│  Or via Emitter::emit_dyn_trait_method_call():                │
│    emitter.emit_dyn_trait_method_call(dynptr_symbol,           │
│                                      slot_index, args, ret_ty)│
└─────────────────────────────────────────────────────────────┘
```

## Vtable Layout

```
@.vtable.<trait>.<type> = private unnamed_addr constant
    [N x ptr] [
        ptr @landin_<type>_<method_0>,
        ptr @landin_<type>_<method_1>,
        ...
    ]

Slot index = method declaration order in trait definition.

@.dynptr.<trait>.<type> = private unnamed_addr constant
    { ptr, ptr } {
        ptr @.data.<type>,        ← data pointer (concrete value)
        ptr @.vtable.<trait>.<type>  ← vtable pointer
    }
```

## Copy Detection (Field-Level Derivation)

```
TraitResolver.collect():
    For each struct/enum S:
      if ALL fields are Copy AND no impl Drop for S:
        derived_copy_types.insert(S.def_id)

ty_is_copy_with_resolver(ty, resolver, interner):
    match ty.kind:
      Int/Uint/Float/Bool/Char → true (primitives)
      Adt(def_id, _) → resolver.is_copy_builtin(def_id) (checks derived_copy_types + explicit impl Copy)
      Closure(_, substs) → substs.all(|t| ty_is_copy_with_resolver(t)) (Stage 16.29)
      Tuple(tys) → tys.all(|t| ty_is_copy_with_resolver(t))
      Array(inner, _) → ty_is_copy_with_resolver(inner)
      Ref(_, _, _) → true (references are always Copy)
      _ → false
```

## DefId-Keyed Lookup (Task 3)

```
Old (deprecated):                   New (Task 3):
impl_by_trait_and_type:              impls_by_def_ids:
  HashMap<(Spur, Spur), DefId>         HashMap<(DefId, DefId), DefId>

  find_impl(trait_spur, type_spur)     find_impl_by_def_ids(trait_def_id, type_def_id)

  Problems:                            Advantages:
  - Spur is string interner ID         - DefId is type-safe unique ID
  - Requires &Rodeo to use             - No interner needed
  - Not type-safe                      - Ready for generic SubstsRef
```

---

## Stage 61 (v0.611.0) — Display trait addition

**Added**: `Display` trait in prelude with `fn fmt(&self, f: &mut String) -> i64` signature.
**Impls**: i32, i64, usize (call `__landin_i64_format`), bool (push_str "true"/"false"), str (push_str self).

**TextEmitter @.data.<type> dedup fix** (Stage 61):
- Before: `emit_dyn_trait_const` emitted `@.data.<type>` once per vtable. With Clone + Display per type, `@.data.i32` was emitted twice → `llvm-as` error: "redefinition of global".
- After: `data_globals_emitted: HashSet<String>` field tracks emitted data globals. `emit_dyn_trait_const` checks the set before emitting.
- Mirrors LLVMSysEmitter's `LLVMGetNamedGlobal` check (llvm/module.rs:197).
- Per §12 (最优 > 最小): root-cause fix — dedup at emission time.
- Per §1.0 原則 6 (通解 > 特解): one mechanism handles all data globals.

**Deferred** (documented as separate TDs):
- `format!` param redesign (`&[i64]` → `&[&dyn Display]`) — needs full `dyn Trait` support (v0.8+)
- `to_string` convenience method — Bug Z7 workaround triggers intermittent LLVM codegen crash (TD-TOSTRING-DEFAULT-BODY, P3, v0.8+)
- TD-TRAIT-NAME-COLLISION — resolver should merge prelude/user trait definitions (P3, v0.8+)

**Test impact**: 7 test/conformance files renamed `Display` → `Show` (TD-TRAIT-NAME-COLLISION workaround, same pattern as Stage 59 Clone→Display rename).
