# Codegen Module Architecture (Post-Stage 16.35)

> **Date**: 2026-08-04
> **Version**: v0.232.0

## Module Structure

```
src/codegen/
├── mod.rs                    (944 LOC) — Entry points + shared pipeline
├── emitter.rs                (530 LOC) — Emitter trait + shared type helpers
├── mir_translation.rs        (994 LOC) — MIR → EmitType translation (shared)
├── operand.rs                (340 LOC) — Operand codegen (shared)
├── rvalue.rs                 (700 LOC) — Rvalue codegen (shared)
├── statement.rs              (700 LOC) — Statement codegen (shared)
├── terminator.rs             (700 LOC) — Terminator codegen (shared)
├── dyn_trait_emit.rs         (400 LOC) — Dyn Trait text helpers
├── text/
│   └── mod.rs                (830 LOC) — TextEmitter + text-backend utilities
├── llvm/
│   └── mod.rs               (2150 LOC) — LLVMSysEmitter (LLVM C-API)
└── trait_dispatch/
    ├── mod.rs                — Trait dispatch orchestrator
    ├── vtable.rs             — Vtable emission
    ├── dynptr.rs             — Dynptr emission
    └── orchestrator.rs       — Combined vtable+dynptr orchestrator
```

## Architecture Layers

```
┌─────────────────────────────────────────────────────────────┐
│                    Entry Points (mod.rs)                      │
│  codegen_crate()          codegen_crate_to_module()          │
│  (Text backend)           (LLVM backend)                     │
└─────────────┬─────────────────────┬──────────────────────────┘
              │                     │
              ▼                     ▼
┌─────────────────────────────────────────────────────────────┐
│              Shared Pipeline (mod.rs)                         │
│  codegen_from_mir() → codegen_function()                     │
│  codegen_synthesized_closure_functions()                     │
│  emit_drop_glue_functions()                                  │
│  Operates on &mut dyn Emitter (backend-agnostic)             │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│         Shared Codegen Primitives (operand/rvalue/etc.)      │
│  codegen_operand()   codegen_rvalue()                        │
│  codegen_statement() codegen_terminator()                    │
│  All take &mut dyn Emitter                                   │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│              MIR Translation (mir_translation.rs)             │
│  mir_type_to_emit_type_with_layouts() — MIR Ty → EmitType   │
│  detect_place_type, detect_operand_type, etc.               │
│  Pure data translation, no Emitter interaction               │
└─────────────────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Emitter Trait                              │
│  (emitter.rs — 39 methods after Stage 16.35 cleanup)         │
│  emit_header, emit_function_begin, emit_binop, emit_load,   │
│  emit_store, emit_call, emit_gep_field, ...                  │
└──────┬──────────────────────────────────┬────────────────────┘
       │                                  │
       ▼                                  ▼
┌──────────────────┐              ┌──────────────────┐
│   TextEmitter     │              │ LLVMSysEmitter    │
│   (text/mod.rs)   │              │ (llvm/mod.rs)    │
│                   │              │                   │
│  emit_type_to_    │              │  llvm_type()      │
│  llvm_str()       │              │  (LLVMTypeRef)    │
│  binop_to_        │              │  LLVMBuildAdd()   │
│  llvm_str()       │              │  etc.             │
│                   │              │                   │
│  output_with_     │              │  to_module()      │
│  globals()        │              │  to_object_file() │
└──────────────────┘              └──────────────────┘
   Text backend                     LLVM C-API backend
   (LLVM IR text)                   (LLVM module → object)
```

## Stage 16.35 Changes

### Moved to text/mod.rs (text-backend-specific):
- `emit_type_to_llvm_str` — renders EmitType as LLVM IR type string
- `binop_to_llvm_str` — renders BinOp as LLVM IR instruction string

### Removed (dead code):
- `emit_dyn_trait_ptr_type` — never called, replaced by inline construction
- `llvm_ptr_str` — never called
- `to_context` (LLVMSysEmitter) — never called
- `predeclare_function` (LLVMSysEmitter) — never called, `#[allow(dead_code)]`

### Fixed (compile bug):
- `codegen_synthesized_closure_functions` — removed incorrect `#[cfg(feature = "llvm-backend")]` gate
  (the function is backend-agnostic, operates on `&mut dyn Emitter`)

## Design Principles

- **§1.0 原則 5 "去除兼容思维"**: Dead code removed, no `#[allow(dead_code)]` except for trait methods
- **§1.0 原則 6 "通用 > 特例"**: Each backend owns its own rendering logic
- **§23 rule 5 (DRY)**: No duplicate type-rendering logic in shared module
- **§16**: Codegen reads MIR data (no HIR access); backend-specific code isolated
