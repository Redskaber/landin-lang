# Closure Data Flow (HIR → MIR → Codegen)

> **Date**: 2026-08-04
> **Version**: v0.232.0
> **Stage**: 16.35 (post-cleanup)

## Closure Pipeline Overview

```
HIR: || x + 1
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  MIR Lowering (mir/lower/expr_operand.rs)                    │
│                                                               │
│  1. Collect captures (closure_capture.rs)                    │
│     captured: Vec<(HirId, LocalId)>                          │
│                                                               │
│  2. Build closure struct type                                │
│     closure_ty = TyKind::Closure(def_id, capture_tys)        │
│                                                               │
│  3. Allocate closure local + assign Aggregate                │
│     closure_local = Aggregate(Closure(def_id, substs),       │
│                               capture_operands)              │
│                                                               │
│  4. Register SynthesizedClosureFunction                       │
│     { def_id, params, body, captures: Vec<(HirId, u32,       │
│       Ty, Mutability)>, closure_struct_ty, fn_name }         │
│                                                               │
│  5. Type-based dispatch at call site:                         │
│     if func_local.ty is Closure(_, _):                        │
│       → lower_closure_call_to_synthesized()                  │
│       → emits TerminatorKind::Call to closure_call_fn_N      │
└─────────────┬────────────────────────────────────────────────┘
              │ SynthesizedClosureFunction
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Closure MIR Body Building (mir/lower/mod.rs)                │
│                                                               │
│  build_synthesized_closure_mir_body(func, interner, hir,     │
│    shared_unify, closure_def_id_counter)                     │
│    → (MirBody, unify, errors, nested_closures, counter)      │
│                                                               │
│  MIR layout:                                                  │
│    LocalId(0) = return local (Mutable)                       │
│    LocalId(1) = self (closure struct, ptr in codegen)        │
│    LocalId(2+) = closure params                              │
│    LocalId(N+) = capture extract locals (with mutability)    │
│    LocalId(M+) = body temp locals                            │
│                                                               │
│  Capture extraction:                                          │
│    cap_local = Copy(Projection(Projection(self, Deref),      │
│                                Field(i, cap_ty)))             │
└─────────────┬────────────────────────────────────────────────┘
              │ MirBody (closure)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Typeck (typeck/checker.rs) — Iterative Fixpoint             │
│                                                               │
│  Pass 1: Typeck all closures + main body                     │
│  Pass 2+: Re-typeck with resolved capture types              │
│    (clear_bindings + re-resolve)                              │
│    Stop at fixpoint (fn_sigs unchanged) or max 4 passes      │
│                                                               │
│  Closure-typed func in check_terminator:                     │
│    if TyKind::Closure(def_id, _):                             │
│      look up sig in fn_sigs                                   │
│      skip first input (self)                                  │
│      unify args with sig params                               │
│      unify dest with sig output                               │
└─────────────┬────────────────────────────────────────────────┘
              │ MirBody (types resolved)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Borrowck (borrowck/mod.rs)                                   │
│                                                               │
│  check_mir_body_with_dataflow(&closure_mir)                   │
│    - Capture extract locals have mutability from outer scope │
│    - Allows `x += 1` where x is a captured `mut`             │
│    - Detects use-after-move, double-mut-borrow               │
└─────────────┬────────────────────────────────────────────────┘
              │ MirBody (validated)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  Codegen (codegen/terminator.rs + codegen/mod.rs)            │
│                                                               │
│  codegen_synthesized_closure_functions():                     │
│    For each closure MirBody:                                  │
│      param_count = fn_sigs[def_id].inputs.len()              │
│      codegen_function(emitter, fn_name, mir, ...)             │
│                                                               │
│  Call site (terminator.rs):                                   │
│    if func operand type is Closure(def_id, _):                │
│      fn_name = fn_name_by_def_id[def_id]                     │
│      PREPEND closure struct as self (first arg)              │
│      emit_call(fn_name, [self, args...], ret_ty)             │
│                                                               │
│  Backend:                                                     │
│    TextEmitter → LLVM IR text (closure_call_fn_N function)   │
│    LLVMSysEmitter → LLVM module (closure_call_fn_N function) │
└─────────────────────────────────────────────────────────────┘
```

## Closure Copy Derivation

```
TyKind::Closure(_, substs)
    │
    ▼
is_copy_closure = substs.iter().all(|t| ty_is_copy_with_resolver(t))
    │
    ├── All captures Copy → Closure is Copy
    │   (allows f()() patterns where f returns a closure)
    │
    └── Any capture non-Copy → Closure is not Copy
        (borrowck enforces Move semantics)
```

## Nested Closure Typeck (Iterative Fixpoint)

```
|| || || x  (triple-nested)

Pass 0:
  1. Typeck inner closure (|| x) → return type = Infer
  2. Typeck middle closure (|| || x) → return type = Closure(inner)
  3. Typeck outer closure (|| || || x) → return type = Closure(middle)
  4. Typeck main body → resolves x: i32

Pass 1 (clear_bindings + re-typeck):
  1. Typeck inner closure → return type = i32 (capture x now resolved)
  2. Typeck middle closure → return type = Closure(inner) ✓
  3. Typeck outer closure → return type = Closure(middle) ✓
  4. Typeck main body → Call sites resolve correctly ✓

Fixpoint: fn_sigs unchanged → stop
```
