# Emitter Trait Hierarchy

> **Date**: 2026-08-04
> **Version**: v0.234.0
> **Status**: Physical split deferred (Stage 16.38), documentation groups active

## Current Trait Structure

```
pub trait Emitter {
    // === Module-level === (5 methods)
    // Survive across function boundaries. Correspond to LLVM's ModuleRef.
    fn emit_header(&mut self);
    fn emit_declare(&mut self, signature: &str);
    fn emit_string_global(&mut self, bytes: &[u8]) -> EmitValue;
    fn emit_vtable_global(&mut self, global_name: &str, method_symbols: &[String]) -> EmitValue;
    fn emit_dyn_trait_const(&mut self, ...) -> EmitValue;

    // === Function scope === (30 methods)
    // Only valid between emit_function_begin and emit_function_end.
    // Correspond to LLVM's BuilderRef.
    fn emit_function_begin(&mut self, ...);
    fn emit_function_end(&mut self);
    fn emit_const(&mut self, ...) -> EmitValue;
    fn emit_binop(&mut self, ...) -> EmitValue;
    fn emit_unop(&mut self, ...) -> EmitValue;
    fn emit_ret(&mut self, ...);
    fn emit_unreachable(&mut self);
    fn emit_br(&mut self, ...);
    fn emit_br_cond(&mut self, ...);
    fn emit_block(&mut self, ...);
    fn emit_switch(&mut self, ...);
    fn emit_alloca(&mut self, ...) -> EmitValue;
    fn emit_store(&mut self, ...);
    fn emit_load(&mut self, ...) -> EmitValue;
    fn emit_call(&mut self, ...) -> EmitValue;
    fn emit_dyn_trait_method_call(&mut self, ...) -> EmitValue;
    fn emit_icmp(&mut self, ...) -> EmitValue;
    fn emit_fcmp(&mut self, ...) -> EmitValue;
    fn emit_and(&mut self, ...) -> EmitValue;
    fn emit_or(&mut self, ...) -> EmitValue;
    fn emit_zext(&mut self, ...) -> EmitValue;
    fn emit_cast(&mut self, ...) -> EmitValue;
    fn emit_select(&mut self, ...) -> EmitValue;
    fn emit_gep_field(&mut self, ...) -> EmitValue;
    fn emit_gep_index(&mut self, ...) -> EmitValue;
    fn emit_gep_index_ptr(&mut self, ...) -> EmitValue;
    fn emit_phi(&mut self, ...) -> EmitValue;
    fn emit_insertvalue(&mut self, ...) -> EmitValue;
    fn emit_extractvalue(&mut self, ...) -> EmitValue;
    fn emit_checked_binop(&mut self, ...) -> EmitValue;

    // === Local state === (4 methods)
    // Function-scoped state management.
    fn set_local_ptr(&mut self, ...);
    fn get_local_ptr(&self, ...) -> Option<&EmitValue>;
    fn set_local(&mut self, ...);
    fn get_local(&self, ...) -> Option<&EmitValue>;
}
```

## Planned Split (Deferred — Stage 16.38)

```
// Module-level operations (mirrors LLVM ModuleRef)
pub trait ModuleEmitter {
    fn emit_header(&mut self);
    fn emit_declare(&mut self, signature: &str);
    fn emit_string_global(&mut self, bytes: &[u8]) -> EmitValue;
    fn emit_vtable_global(&mut self, ...) -> EmitValue;
    fn emit_dyn_trait_const(&mut self, ...) -> EmitValue;
}

// Function-scoped operations (mirrors LLVM BuilderRef)
pub trait FunctionEmitter {
    fn emit_function_begin(&mut self, ...);
    // ... 30 instruction/state methods ...
}

// Combined trait for backward compatibility
pub trait Emitter: ModuleEmitter + FunctionEmitter {}
```

## Why Deferred

Rust does not allow multiple `impl` blocks for the same trait on the same
type. The current impl blocks have module-level and function-scoped methods
interleaved. Splitting requires physically moving ~1000 lines of method
implementations across both `text/mod.rs` and `llvm/mod.rs`.

Per §1.0 原則 9 "正确 > 妥协": correct long-term design, but code movement
risk is too high for this stage.

## Implementations

| Backend | Impl Block | Output Method |
|---------|-----------|---------------|
| TextEmitter | `impl Emitter for TextEmitter` in `text/mod.rs` | `output_with_globals() → String` |
| LLVMSysEmitter | `impl Emitter for LLVMSysEmitter` in `llvm/mod.rs` | `to_module() / to_object_file()` |

## EmitValue Type

```rust
pub type EmitValue = String;
```

**Current**: `EmitValue = String` — text-IR assumption that forces the LLVM
backend to maintain `HashMap<String, LLVMValueRef>` and parse strings back
into `LLVMValueRef`s via `interpret_adhoc()`.

**Future (deferred)**: Replace with opaque associated type (`type Value: Clone`)
to eliminate the string-to-pointer bookkeeping in the LLVM backend.
