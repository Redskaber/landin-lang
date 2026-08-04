# Codegen Data Flow (MIR → LLVM IR)

> **Date**: 2026-08-04
> **Version**: v0.234.0

## Unified Pipeline Data Flow

```
CompileResult (from driver)
    │
    │  mirs: Vec<MirBody>
    │  body_metas: Vec<BodyMeta>
    │  fn_name_by_def_id: HashMap<DefId, String>
    │  fn_sigs: HashMap<DefId, Sig>
    │  interner: Rodeo
    │  trait_resolver: TraitResolver
    │  synthesized_closure_mir_bodies: Vec<MirBody>
    │  adt_layouts: AdtLayouts
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  run_codegen_pipeline(result, &mut dyn Emitter)                 │
│                                                                   │
│  Step 1: Module Header                                           │
│    emitter.emit_header()                                         │
│    emitter.emit_declare("void @__landin_panic_overflow(...)")   │
│    emitter.emit_declare("void @__landin_panic_bounds_check(...)")│
│    emitter.emit_declare("void @__landin_panic_div_by_zero()")   │
│                                                                   │
│  Step 2: Vtable Globals (BEFORE function bodies)                │
│    emit_vtables(&trait_resolver, &interner, emitter)             │
│      → emitter.emit_vtable_global(name, method_symbols)         │
│                                                                   │
│  Step 3: Dyn Trait Fat-Pointer Globals                           │
│    emit_dyn_trait_ptrs(&trait_resolver, &interner, emitter)      │
│      → emitter.emit_dyn_trait_const(name, data, vtable)         │
│                                                                   │
│  Step 4: Drop Glue Functions                                     │
│    emit_drop_glue_functions(&resolver, &interner, ...)           │
│      → emitter.emit_function_begin/end (drop_adt_N functions)    │
│                                                                   │
│  Step 5: Main MIR Function Bodies                                │
│    codegen_from_mir(&mirs, &metas, ..., emitter)                 │
│      for each (mir, meta) in mirs.zip(metas):                   │
│        codegen_function(emitter, meta.fn_name, mir, ...)         │
│          ├─ emitter.emit_function_begin(name, params, ret)      │
│          ├─ for each basic block:                                │
│          │    emitter.emit_block(label)                          │
│          │    for each statement:                                │
│          │      codegen_statement(emitter, mir, stmt)            │
│          │        → emitter.emit_alloca / emit_store / emit_binop│
│          │    codegen_terminator(emitter, mir, term)             │
│          │        → emitter.emit_br / emit_ret / emit_call      │
│          └─ emitter.emit_function_end()                          │
│                                                                   │
│  Step 6: Synthesized Closure Functions                           │
│    codegen_synthesized_closure_functions(&closures, ..., emitter)│
│      for each closure_mir:                                       │
│        codegen_function(emitter, fn_name, closure_mir, ...)      │
│                                                                   │
└──────────────────────────┬──────────────────────────────────────┘
                           │
              ┌────────────┴────────────┐
              ▼                         ▼
┌──────────────────────┐    ┌──────────────────────┐
│    TextEmitter        │    │   LLVMSysEmitter      │
│                       │    │                       │
│  Buffers:             │    │  In-memory:           │
│  output: String       │    │  LLVMModuleRef        │
│  globals: Vec<String> │    │  LLVMBuilderRef       │
│                       │    │  values: HashMap      │
│  Output:              │    │    <String, ValueRef> │
│  output_with_globals()│    │                       │
│  → String (LLVM .ll)  │    │  Output:              │
│                       │    │  to_object_file()     │
│                       │    │  → .o file             │
└──────────────────────┘    └──────────────────────┘
```

## MIR → EmitType Translation (mir_translation.rs)

```
MIR Ty                          EmitType
──────                          ────────
TyKind::Int(I32)          →     EmitType::I32
TyKind::Uint(U64)         →     EmitType::I64
TyKind::Float(F64)        →     EmitType::F64
TyKind::Bool              →     EmitType::I1
TyKind::Ref(_, _, inner)  →     EmitType::Ptr(Box<inner>)
TyKind::Tuple(tys)        →     EmitType::Struct(tys)
TyKind::Array(elem, n)    →     EmitType::Array(Box<elem>, n)
TyKind::Adt(def_id, _)    →     EmitType::Struct(field_tys)  (via AdtLayouts)
TyKind::Closure(_, substs)→     EmitType::Struct(substs)
TyKind::FnDef(_, _)       →     EmitType::OpaquePtr
TyKind::FnPtr(_)          →     EmitType::OpaquePtr
TyKind::Str               →     EmitType::Ptr(I8)
TyKind::Slice(elem)       →     EmitType::Ptr(elem)
```

## Key Data Structures

```
EmitType (enum, non-Copy):
  I1 | I8 | I16 | I32 | I64 | I128 | F32 | F64
  | Ptr(Box<EmitType>) | OpaquePtr | Void
  | Struct(Vec<EmitType>) | Array(Box<EmitType>, u64)

EmitValue = String  (e.g., "%v3", "@.str.0", "42", "undef")
```
