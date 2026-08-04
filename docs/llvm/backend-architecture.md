# LLVM Backend Architecture

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.234.0
> **Status**: Post-codegen refactoring (Stages 16.35-16.40)

## Overview

The Landin compiler uses LLVM 19 via the `llvm-sys` crate (C API bindings)
to produce native object code. The LLVM backend is implemented in
`src/codegen/llvm/mod.rs` as `LLVMSysEmitter`, which implements the
`Emitter` trait.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    LLVMSysEmitter                             │
│                    (llvm/mod.rs)                              │
│                                                               │
│  LLVM State:                                                 │
│  ├── ctx: LLVMContextRef       (type/constant allocation)   │
│  ├── module: LLVMModuleRef     (global container)            │
│  ├── builder: LLVMBuilderRef   (instruction builder)         │
│  │                                                            │
│  ├── values: HashMap<String, LLVMValueRef>                   │
│  │   (EmitValue key → LLVM value pointer)                    │
│  ├── local_ptrs: HashMap<u32, EmitValue>                     │
│  ├── locals: HashMap<u32, EmitValue>                         │
│  ├── blocks: HashMap<String, LLVMBasicBlockRef>              │
│  ├── declared: HashMap<String, LLVMValueRef>                 │
│  │   (function forward declarations)                         │
│  ├── struct_type_cache: RefCell<HashMap<String, LLVMTypeRef>>│
│  └── fn_sigs: HashMap<String, (EmitType, Vec<EmitType>)>    │
│      (forward reference signatures)                          │
│                                                               │
│  Key Methods:                                                │
│  ├── new() → creates ctx, module, builder                    │
│  ├── to_module() → LLVMModuleRef                             │
│  ├── to_object_file(path) → Result<(), String>               │
│  ├── set_fn_sigs(sigs) → populate fn_sigs map               │
│  ├── get_or_declare_function(name, ret, args) → ValueRef     │
│  ├── llvm_type(EmitType) → LLVMTypeRef                       │
│  └── impl Emitter (39 methods)                               │
└─────────────────────────────────────────────────────────────┘
```

## EmitValue Bridge

The `Emitter` trait uses `EmitValue = String`. The LLVM backend bridges
this by:

1. Each `emit_*` call creates an `LLVMValueRef` via the C API
2. The value is assigned a unique `"%vN"` name (via `fresh_named()`)
3. The name is stored in `values: HashMap<String, LLVMValueRef>`
4. The name (String) is returned as the `EmitValue`
5. When a later call receives an `EmitValue`, it calls `lookup()` to
   retrieve the `LLVMValueRef`

The `interpret_adhoc()` method handles special string values like
`"0"`, `"undef"`, `"null"`, and GEP expressions that don't have a
pre-assigned `"%vN"` name.

## Object File Generation

```
LLVMSysEmitter::to_object_file(out_path):
    1. Initialize LLVM targets (LLVMInitializeAllTargets, etc.)
    2. Get host triple (LLVMGetDefaultTargetTriple)
    3. Get target (LLVMGetTargetFromTriple)
    4. Create target machine (LLVMCreateTargetMachine)
    5. Set module data layout + target triple
    6. Emit to file (LLVMTargetMachineEmitToFile)
       → produces .o object file
    7. Linker (cc/clang) converts .o → executable
```

## Feature Gate

The LLVM backend is behind `#[cfg(feature = "llvm-backend")]`:
- `Cargo.toml`: `[features] llvm-backend = ["llvm-sys"]`
- `src/codegen/llvm/mod.rs`: `#![cfg(feature = "llvm-backend")]`
- Entry point `codegen_crate_to_module()`: `#[cfg(feature = "llvm-backend")]`

Without the feature, only `TextEmitter` is available (frontend-only builds).

## Environment Setup

```bash
# LLVM 19 environment (scripts/setup-llvm-env.sh)
export LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix
export LLVM_LINK_SHARED=1

# Build
cargo build --features llvm-backend

# Test
cargo test --features llvm-backend

# Run
./target/debug/landin-stage0 --run program.lin
```

## Dead Code Removed (Stages 16.35-16.40)

| Item | Description |
|------|-------------|
| `to_context()` | Never called externally |
| `predeclare_function()` | `#[allow(dead_code)]`, never called |
| `emit_output()` impl | Returned `""` — dead trait method |

## Future Improvements (Deferred)

1. **Replace `EmitValue = String`** with opaque associated type to
   eliminate `values: HashMap` + `interpret_adhoc()` string parser
2. **Physical trait split** into `ModuleEmitter` + `FunctionEmitter`
3. **LLVM ORC JIT** for in-process execution (avoid linking step)
