# Codegen Module Architecture (Final — Post-Stage 16.40)

> **Date**: 2026-08-04
> **Version**: v0.234.0
> **Status**: Codegen architecture refactoring COMPLETE (Stages 16.35-16.40)

## Module Structure

```
src/codegen/
├── mod.rs                    — Entry points + unified pipeline (run_codegen_pipeline)
├── emitter.rs                — Emitter trait (39 methods, 3 documentation groups)
│                                + shared type helpers (emit_fat_ptr_type, mir_type_to_emit_type)
├── mir_translation.rs        — MIR → EmitType translation (shared, pure data)
├── operand.rs                — Operand codegen (shared)
├── rvalue.rs                 — Rvalue codegen (shared)
├── statement.rs              — Statement codegen (shared)
├── terminator.rs             — Terminator codegen (shared)
├── dyn_trait_emit.rs         — Dyn Trait text rendering helpers (test-only, not re-exported)
├── text/
│   └── mod.rs                — TextEmitter + text-backend utilities
│                                (emit_type_to_llvm_str, binop_to_llvm_str)
├── llvm/
│   └── mod.rs                — LLVMSysEmitter (LLVM C-API, own type rendering)
└── trait_dispatch/
    ├── mod.rs                — Trait dispatch orchestrator
    ├── vtable.rs             — Vtable emission
    ├── dynptr.rs             — Dynptr emission
    └── orchestrator.rs       — Combined vtable+dynptr orchestrator
```

## Architecture Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                    Entry Points (mod.rs)                          │
│                                                                   │
│  codegen_crate()               codegen_crate_to_module()         │
│  (Text backend → String)       (LLVM backend → LLVMSysEmitter)   │
│                                                                   │
│  Both delegate to run_codegen_pipeline():                         │
│    1. emit_header + panic declares                               │
│    2. emit_vtables (globals BEFORE functions)                    │
│    3. emit_dyn_trait_ptrs                                        │
│    4. emit_drop_glue_functions                                  │
│    5. codegen_from_mir (main MIR function bodies)               │
│    6. codegen_synthesized_closure_functions                     │
│                                                                   │
│  Stage 16.37: Unified pipeline — one emission order for ALL      │
│  backends. Text buffers globals separately, appends at output.   │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│              Shared Codegen Primitives                            │
│                                                                   │
│  codegen_from_mir() → codegen_function()                         │
│    ├─ codegen_statement()  (statement.rs)                        │
│    ├─ codegen_terminator() (terminator.rs)                       │
│    ├─ codegen_operand()    (operand.rs)                          │
│    └─ codegen_rvalue()     (rvalue.rs)                           │
│                                                                   │
│  All operate on &mut dyn Emitter (backend-agnostic)              │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│              MIR Translation (mir_translation.rs)                 │
│                                                                   │
│  mir_type_to_emit_type_with_layouts() — MIR Ty → EmitType       │
│  detect_place_type, detect_operand_type, etc.                    │
│  Pure data translation, no Emitter interaction                   │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Emitter Trait (emitter.rs)                     │
│                                                                   │
│  39 methods organized into 3 documentation groups:               │
│                                                                   │
│  // === Module-level === (5 methods)                             │
│  emit_header, emit_declare,                                      │
│  emit_string_global, emit_vtable_global, emit_dyn_trait_const   │
│                                                                   │
│  // === Function scope === (30 methods)                          │
│  emit_function_begin, emit_function_end,                         │
│  emit_const, emit_binop, emit_unop,                             │
│  emit_ret, emit_unreachable, emit_br, emit_br_cond,             │
│  emit_block, emit_switch, emit_alloca, emit_store, emit_load,   │
│  emit_call, emit_dyn_trait_method_call,                          │
│  emit_icmp, emit_fcmp, emit_and, emit_or,                       │
│  emit_zext, emit_cast, emit_select,                              │
│  emit_gep_field, emit_gep_index, emit_gep_index_ptr,            │
│  emit_phi, emit_insertvalue, emit_extractvalue,                 │
│  emit_checked_binop                                              │
│                                                                   │
│  // === Local state === (4 methods)                              │
│  set_local_ptr, get_local_ptr, set_local, get_local             │
│                                                                   │
│  Stage 16.38: Physical trait split into ModuleEmitter +          │
│  FunctionEmitter deferred (blocked by Rust's single-impl-block   │
│  rule). Documentation groups provide architectural clarity.      │
└──────┬──────────────────────────────────────┬───────────────────┘
       │                                      │
       ▼                                      ▼
┌──────────────────────┐              ┌──────────────────────┐
│    TextEmitter        │              │   LLVMSysEmitter      │
│    (text/mod.rs)      │              │   (llvm/mod.rs)      │
│                       │              │                       │
│  Text-backend utils:  │              │  LLVM C-API:          │
│  emit_type_to_        │              │  llvm_type()          │
│    llvm_str()         │              │    (→ LLVMTypeRef)    │
│  binop_to_            │              │  LLVMBuildAdd()       │
│    llvm_str()         │              │  LLVMBuildLoad()      │
│                       │              │  etc.                 │
│  Output:              │              │                       │
│  output_with_         │              │  Output:              │
│    globals() → String │              │  to_module()          │
│                       │              │  to_object_file()     │
└──────────────────────┘              └──────────────────────┘
   Text backend                         LLVM C-API backend
   (LLVM IR text .ll)                   (LLVM module → .o)
```

## Unified Pipeline (Stage 16.37)

```
run_codegen_pipeline(result, &mut dyn Emitter):
    │
    ├─ 1. emit_header + emit_declare (3 panic functions)
    ├─ 2. emit_vtables (globals BEFORE function bodies)
    ├─ 3. emit_dyn_trait_ptrs (dyn Trait fat-pointer globals)
    ├─ 4. emit_drop_glue_functions (Drop::drop wrappers)
    ├─ 5. codegen_from_mir (main MIR → LLVM IR function bodies)
    └─ 6. codegen_synthesized_closure_functions (closure call functions)

Entry points:
  codegen_crate:          TextEmitter::new() → pipeline → output_with_globals()
  codegen_crate_to_module: LLVMSysEmitter::new() → set_fn_sigs → pipeline → return emitter
```

## Dead Code Cleanup (Stages 16.35-16.40)

| Item | Removed In |
|------|-----------|
| `emit_output` (trait method) | 16.36 |
| `emit_dyn_trait_ptr_type` | 16.35 |
| `llvm_ptr_str` | 16.35 |
| `to_context` (LLVMSysEmitter) | 16.35 |
| `predeclare_function` (LLVMSysEmitter) | 16.35 |
| 7 dead `dyn_trait_emit` re-exports | 16.40 |

**Zero dead code in codegen module.**

## Design Principles Applied

- **§1.0 原則 5 "去除兼容思维"**: All dead code removed
- **§1.0 原則 6 "通用 > 特例"**: Unified pipeline for all backends
- **§23 rule 5 (DRY)**: No duplicate type-rendering logic in shared module
- **§16**: Codegen reads MIR data (no HIR access); backend-specific code isolated
