# Text Backend vs LLVM C-API Backend Comparison

> **Date**: 2026-08-04
> **Version**: v0.234.0

## Overview

The Landin compiler has two codegen backends, both implementing the `Emitter`
trait. They produce the same LLVM IR but through fundamentally different
mechanisms.

## Architecture Comparison

```
                     ┌─────────────────────────────────────────┐
                     │           Emitter Trait                  │
                     │     (39 methods, 3 doc groups)           │
                     └─────────────┬─────────────┬─────────────┘
                                   │             │
                    ┌──────────────▼──┐  ┌───────▼──────────────┐
                    │   TextEmitter    │  │  LLVMSysEmitter       │
                    │   (text/mod.rs)  │  │  (llvm/mod.rs)        │
                    └──────────────────┘  └───────────────────────┘
```

## Detailed Comparison

| Aspect | TextEmitter | LLVMSysEmitter |
|--------|-------------|----------------|
| **Mechanism** | String concatenation | LLVM C-API calls |
| **Output** | LLVM IR text (.ll) | LLVM module → object file (.o) |
| **Value representation** | `String` (e.g., `"%v3"`) | `String` key → `LLVMValueRef` map |
| **Type rendering** | `emit_type_to_llvm_str()` → `String` | `llvm_type()` → `LLVMTypeRef` |
| **Binary op** | `binop_to_llvm_str()` → text like `"add nsw i32"` | `LLVMBuildAdd()` → `LLVMValueRef` |
| **Output retrieval** | `output_with_globals()` → `String` | `to_module()` / `to_object_file()` |
| **Forward refs** | Text can reference undeclared symbols | `fn_sigs_map` for forward declarations |
| **Global storage** | `globals: Vec<String>` (appended at end) | `LLVMAddGlobal()` (in module) |
| **Feature gate** | Always available | `#[cfg(feature = "llvm-backend")]` |
| **Dependencies** | None (pure Rust) | `llvm-sys` crate (LLVM C API) |

## Key Differences

### 1. Value Representation

**TextEmitter**: `EmitValue = String` — values are LLVM IR text fragments
like `"%v3"`, `"@.str.0"`, `"42"`. The emitter builds a text buffer by
concatenating these strings.

**LLVMSysEmitter**: `EmitValue = String` — but the string is a *key* into
`values: HashMap<String, LLVMValueRef>`. Each `emit_*` call creates an
`LLVMValueRef`, assigns it a unique `"%vN"` name, and stores the mapping.
When a later `emit_*` call receives an `EmitValue` (String key), it looks
up the `LLVMValueRef` via `lookup()`.

### 2. Type Rendering

**TextEmitter** uses `emit_type_to_llvm_str()` (in `text/mod.rs`):
```rust
fn emit_type_to_llvm_str(ty: &EmitType) -> String {
    match ty {
        EmitType::I32 => "i32".into(),
        EmitType::Ptr(_) | EmitType::OpaquePtr => "ptr".into(),
        EmitType::Struct(fields) => format!("{{ {} }}", ...),
        ...
    }
}
```

**LLVMSysEmitter** uses `llvm_type()` (in `llvm/mod.rs`):
```rust
fn llvm_type(&self, ty: &EmitType) -> LLVMTypeRef {
    match ty {
        EmitType::I32 => LLVMInt32TypeInContext(self.ctx),
        EmitType::Ptr(_) | EmitType::OpaquePtr => LLVMPointerType(...),
        EmitType::Struct(fields) => LLVMStructTypeInContext(...),
        ...
    }
}
```

Both are in their respective backend modules (Stage 16.35 moved
`emit_type_to_llvm_str` from `emitter.rs` to `text/mod.rs`).

### 3. Global Emission Order

**Unified pipeline** (Stage 16.37): Both backends use `run_codegen_pipeline`
which emits globals BEFORE function bodies. This works because:
- LLVM C-API needs forward declarations before function bodies reference them
- TextEmitter buffers globals in `globals: Vec<String>` and appends them
  at output time via `output_with_globals()` — text IR allows globals
  before function definitions

### 4. Forward Reference Resolution

**TextEmitter**: Text IR allows referencing undeclared symbols (they just
need to be declared somewhere in the module). No special handling needed.

**LLVMSysEmitter**: Uses `fn_sigs_map: HashMap<String, (EmitType, Vec<EmitType>)>`
to create forward declarations with correct signatures before function
bodies are emitted. Set via `set_fn_sigs()` before `run_codegen_pipeline()`.

## Performance

| Metric | TextEmitter | LLVMSysEmitter |
|--------|-------------|----------------|
| **Compilation speed** | Fast (string concat) | Slower (C API calls) |
| **Output size** | Large (.ll text) | Compact (binary .o) |
| **Linking** | Requires `llc` + `cc` | Direct (LLVM handles) |
| **Runtime** | Via `llc | cc | run` | Via `--run` (direct) |

## When to Use Each

| Use Case | Backend |
|----------|---------|
| Debugging IR output | TextEmitter (`--emit-llvm-ir`) |
| Producing executables | LLVMSysEmitter (`--emit-bin`, `--run`) |
| Producing object files | LLVMSysEmitter (`--emit-obj`) |
| CI / automated testing | LLVMSysEmitter (conformance tests) |
| Frontend-only builds | TextEmitter (no `llvm-sys` dependency) |
