# Stage 13.6 — LLVM Object File Generation + CLI --emit-obj/--emit-bin

> **Date**: 2026-07-26
> **Version**: v0.24.0 (no bump — feature-gated behind `llvm-backend`)

## What Was Done

### CLI Enhancements (`src/bin/main.rs`)

New flags added:
- `--emit-obj` — Generate object file (.o) via LLVMSysEmitter + LLVM TargetMachine
- `--emit-bin` — Generate executable by linking .o via system `cc`/`clang`
- `-o <FILE>` — Specify output file path

Usage:
```bash
# Object file
landin-stage0 --emit-obj hello.ln -o hello.o

# Executable
landin-stage0 --emit-bin hello.ln -o hello

# Without llvm-backend feature (graceful error)
landin-stage0 --emit-obj hello.ln
# → error: --emit-obj/--emit-bin requires --features llvm-backend
```

### LLVMSysEmitter Object File Generation (verified ✅)

The `to_object_file()` method was verified end-to-end:
- Input: Simple `define i32 @main() { ret i32 42 }` module built via LLVMSysEmitter
- Output: `/tmp/test_simple_module.o` — 768 bytes, ELF 64-bit LSB relocatable, x86-64
- Verified with `file` command: valid ELF object file

### Inline Test (passing ✅)

`test_simple_module_builds_and_emits` in `src/codegen/llvm_sys_emitter.rs`:
1. Creates LLVMSysEmitter
2. Builds a simple `main() -> i32 { 42 }` function
3. Calls `to_object_file("/tmp/test_simple_module.o")`
4. Verifies file exists + is non-empty (768 bytes)

## Known Limitations

1. **End-to-end Landin→object** segfaults when using `codegen_crate_to_module` with a real Landin program — the `codegen_from_mir` → LLVMSysEmitter path has issues with some MIR constructs (likely in `emit_block` or `emit_load`/`emit_store` where TextEmitter and LLVMSysEmitter differ in value tracking)
2. The simple module test (manual function build) works perfectly — the issue is in the `codegen_from_mir` integration, not in the LLVM C API calls
3. `--emit-bin` (linker) is implemented but untested until the Landin→object path is stable

## Next Steps

1. Debug the `codegen_from_mir` → LLVMSysEmitter segfault (likely a null pointer in `lookup()` or `block_for()`)
2. Once Landin→object works, test `--emit-bin` with `cc`
3. Implement runtime library (Stage 13.7) for `println!` support
4. Add `--run` flag (Stage 13.8)
