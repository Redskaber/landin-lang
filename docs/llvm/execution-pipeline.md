# Execution Pipeline — Landin → Object → Link → Run

> **Stage 13.5-13.8**: Complete execution pipeline from Landin source to running binary
> **Verified**: 2026-07-26

## Pipeline Overview

```
Landin source (.ln)
    ↓ driver::compile()
    ↓ (lex → parse → AST → HIR → resolve → MIR → typeck → borrowck)
CompileResult (MIR + metadata)
    ↓ codegen_crate_to_module()
    ↓ (MIR → LLVMSysEmitter via LLVM C API)
LLVMModuleRef
    ↓ to_object_file()
    ↓ (LLVMTargetMachineEmitToFile)
ELF object file (.o)
    ↓ auto-generate C wrapper
    ↓ (extern int landin_main(void); int main(void) { return landin_main(); })
    ↓ cc wrapper.c prog.o -o exe -lm
Executable
    ↓ ./exe
Program exit code = landin_main() return value
```

## CLI Flags

| Flag | Description | Feature Gate |
|------|-------------|-------------|
| `--emit-tokens` | Output token stream | default |
| `--emit-ast` | Output AST summary | default |
| `--compile` | Full compile (verify) | default |
| `--emit-llvm-ir` | Output LLVM IR text | default |
| `--emit-obj` | Generate object file (.o) | `llvm-backend` |
| `--emit-bin` | Generate executable | `llvm-backend` |
| `--run` | Compile + link + execute | `llvm-backend` |
| `-o <FILE>` | Specify output path | default |

## Verified Programs (all correct ✅)

| Program | Expected | Got | Status |
|---------|----------|-----|--------|
| `fn main() -> i32 { 42 }` | 42 | 42 | ✅ |
| `let x = 10; let y = 20; x + y` | 30 | 30 | ✅ |
| `if x > 3 { 100 } else { 200 }` (x=5) | 100 | 100 | ✅ |
| `while i < 10 { sum += i; i++ }` | 45 | 45 | ✅ |
| `match x { 1=>10, 2=>20, _=>30 }` (x=2) | 20 | 20 | ✅ |
| `fib(10)` recursive | 55 | 55 | ✅ |
| `mul(6, 7)` function call | 42 | 42 | ✅ |
| `double(add(3, 4))` nested calls | 14 | 14 | ✅ |
| `sq(3) + cube(2)` multi-function | 17 | 17 | ✅ |
| `struct Point { x: i32, y: i32 } p.x + p.y` | 7 | 7 | ✅ |
| `let t = (10, 20); t.0 + t.1` tuple | 30 | 30 | ✅ |
| `if 5 > 3 { 1 } else { 0 }` boolean | 1 | 1 | ✅ |
| `enum Opt { Some(i32), None } match` | 42 | 42 | ✅ |

## Architecture

### LLVMSysEmitter (src/codegen/llvm_sys_emitter.rs, ~1360 LOC)

Implements the `Emitter` trait using LLVM C API (`llvm-sys` crate):

- `LLVMContextCreate` / `LLVMModuleCreateWithNameInContext` / `LLVMCreateBuilderInContext`
- 36 Emitter trait methods (emit_header, emit_function_begin, emit_binop, emit_ret, etc.)
- `to_object_file()` — uses `LLVMTargetMachineEmitToFile` to produce ELF .o
- `to_module()` — returns raw `LLVMModuleRef` for further processing

### Key Design Decisions

1. **EmitValue = String bridging**: LLVMSysEmitter stores `HashMap<String, LLVMValueRef>`
   mapping SSA names (like `%v1`, `%loc_0`, `%arg0`) to LLVM values. Each `emit_*` method
   creates an LLVM value, stores it under a fresh name, and returns the name.

2. **Entry block reuse**: `emit_block("bb0")` reuses the entry block created by
   `emit_function_begin` (registered as `%entry`), avoiding orphan basic blocks.

3. **Locals cache clearing**: `emit_block` clears the `locals` cache at block boundaries,
   mirroring `TextEmitter` behavior — forces reload from alloca slots.

4. **Auto C wrapper**: `--run` and `--emit-bin` generate a temporary C file with
   `extern int landin_main(void); int main(void) { return landin_main(); }` to provide
   a standard C `main` entry point for the linker.

## Build Instructions

### Build Server (LLVM 19, no root)

```bash
source scripts/setup-llvm-env.sh
cargo build --lib --features llvm-backend
```

### User Environment (LLVM 21)

```bash
bash scripts/switch-llvm-version.sh   # auto-detects LLVM 21
cargo build --lib --features llvm-backend
```

### Run a Program

```bash
echo 'fn main() -> i32 { 42 }' > hello.ln
./target/debug/landin-stage0 --run hello.ln
echo $?  # → 42
```

## Known Limitations

1. **println! produces unit (no output)**: `println!("hello")` compiles but doesn't print.
   Runtime library + printf integration is the next step.
2. **`return` in if-blocks**: Type checking issue with `return` inside `if` blocks
   (mismatched types: expected I32, found unit). To be fixed in future stage.
3. **Closures as values**: Inline closure call works (Stage 13.3a), but passing closures
   as function arguments requires full Strategy A (deferred to Stage 13.5+).
4. **dyn Trait method calls**: `emit_dyn_trait_method_call` is stubbed with `unimplemented!()`.
