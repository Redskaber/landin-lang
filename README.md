# Landin

> A work-in-progress systems programming language inspired by Rust, using
> LLVM 22 (llvm-sys 221) for code generation. The compiler is written in
> Rust (~82,000 LOC) and targets x86_64 + AArch64 Linux.

| | |
|---|---|
| **Author** | redskaber |
| **Version** | v0.506.0 (Stage 18.346) |
| **License** | MIT |
| **Status** | v0.4 stable. 676 lib tests + 3689 integration tests = 4365 total, 0 failures (single-thread, `ulimit -s unlimited`). All P0/P1/P2 tech-debts resolved. §14.5 D1-D8 deep review PASSED. Generic struct field_tys inference: MIR lower now infers adt_substs from operand types when no turbofish. Wrapper<i64> insertvalue produces correct { i64 } struct type. |
| **LLVM** | 22.1.8 (llvm-sys 221) |
| **Rust edition** | 2021 |

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [CLI Reference](#cli-reference)
3. [Language Features](#language-features)
4. [Codegen ABI Compliance](#codegen-abi-compliance)
5. [Testing](#testing)
6. [Project Layout](#project-layout)
7. [Documentation](#documentation)
8. [Roadmap](#roadmap)
9. [Contributing](#contributing)

---

## Quick Start

### Prerequisites

- Rust stable (≥ 1.70.0) + cargo + rustfmt + clippy
- LLVM 22.1 development headers (auto-installed via `scripts/setup-llvm-env.sh`)
- cc/clang (for linking)
- Linux x86_64 or aarch64

### Build

```bash
# 1. Setup LLVM environment (auto-detects LLVM 22, falls back to LLVM 19)
source scripts/setup-llvm-env.sh
# Or use the env.sh helper (sets PATH + LD_LIBRARY_PATH for LLVM 22):
source scripts/env.sh

# 2. Build
cargo build --release --features llvm-backend

# 3. Run tests (auto-tunes --test-threads + raises ulimit -s for LLVM)
bash scripts/run_tests.sh
```

### Hello World

```bash
echo 'fn main() -> i32 { println!("hello world"); 0 }' > hello.lin
./target/release/landin-stage0 --run hello.lin
```

### Multi-File Project (`landinc`)

```bash
landinc new my_project && cd my_project
landinc build --release
landinc run
```

---

## CLI Reference

### `landin-stage0` — single-file compiler

| Flag | Description |
|------|-------------|
| `--compile` | Full pipeline (lex → parse → typeck → borrowck → codegen) |
| `--emit-llvm-ir` | Emit LLVM IR text (implies `--compile`) |
| `--emit-obj` | Emit object file `.o` (requires `llvm-backend`) |
| `--emit-bin` | Emit executable (requires `llvm-backend`) |
| `--run` | Compile, link, and run (requires `llvm-backend`) |
| `--emit-tokens` | Emit token stream only (debug) |
| `--emit-ast` | Emit AST only (debug) |
| `--color WHEN` | Color output: `auto` / `always` / `never` (default: auto) |
| `--target TRIPLE` | Cross-compile target (e.g. `aarch64-unknown-linux-gnu`) |

### `landinc` — multi-file project tool

```bash
landinc new <name>      # Create new project
landinc build [--release]  # Build all .lin files in src/
landinc run             # Build + run
landinc check           # Type-check only (no codegen)
landinc test            # Run unit tests
```

---

## Language Features

Landin implements a Rust-inspired syntax with the following feature set:

### Types
- Primitives: `i8`, `i16`, `i32`, `i64`, `i128`, `u8`, `u16`, `u32`, `u64`, `usize`, `isize`, `f32`, `f64`, `bool`
- Strings: `&str` (fat pointer: `{ ptr, len }`), `String` (owned: `{ ptr, len, cap }`)
- Collections: `Vec<T>`, `Box<T>`, `Option<T>`, `Result<T, E>`
- Arrays: `[T; N]`
- References: `&T`, `&mut T`
- Function pointers: `fn(...) -> T`
- Closures: `|args| expr`
- Trait objects: `dyn Trait`

### Constructs
- `fn` — function definitions
- `struct` — named-field structs (including recursive via pointer)
- `enum` — tagged unions
- `impl` — inherent + trait implementations
- `trait` — trait definitions
- `let` / `let mut` — variable bindings
- `if` / `else` / `match` — control flow
- `while` / `for` / `loop` — loops
- `&` / `&mut` — borrows
- `*` — dereference

### Macros
- `println!` / `print!` / `eprintln!` / `eprint!`
- `format!` (variadic, MIR intrinsic — Stage 18.231)
- `vec!` (Stage 18.229)
- `stringify!`, `concat!`, `panic!`, `assert!`, `assert_eq!`

### Trait dispatch
- Static dispatch (monomorphization — Stage 18.112)
- Dynamic dispatch via `dyn Trait` (vtable indirect call — Stage 5.78)
- Trait objects with `Copy` / `Clone` auto-derivation (Stage 18.202)

### Memory safety
- Ownership + borrow checking (Stage 9.5+)
- Move semantics with flow-sensitive drop elaboration (Stage 18.282)
- Zero-cost abstractions (no runtime overhead for traits, generics)
- Bounds checking on array/string indexing (panics on OOB)

---

## Codegen ABI Compliance

Landin explicitly models System V AMD64 ABI requirements at the LLVM IR
level (rather than relying on LLVM's CodeGenPrepare auto-lowering, which
was found unreliable across LLVM versions — see Stages 18.328/18.329/18.332).

### ABI attributes emitted

| Attribute | When | Where |
|-----------|------|-------|
| `sret(<ty>)` | Function return type > 16 bytes | Param 1 of callee + call site |
| `byval(<ty>)` | Function param type > 16 bytes | Param `i+1+(1 if sret else 0)` of callee + call site |
| `ptr` (opaque) | All pointer types (LLVM 17+ opaque pointer mode) | All GEP/load/store/alloca |

### Implementation sites

Both `LLVMSysEmitter` (production codegen via LLVM C-API) and
`TextEmitter` (`--emit-llvm-ir` debug path) emit identical ABI attributes
at these 6 sites:

1. `emit_function_begin` — function definition signature
2. `declare_function` — forward declaration (call-before-def)
3. `interpret_adhoc` — ad-hoc forward decl from `fn_sigs` map
4. `emit_call` — direct call site
5. `emit_dyn_trait_method_call` — vtable indirect call site
6. `emit_ret` — return instruction (sret only)

### Entry-block allocas

All ABI-required stack slots (sret return slot, byval arg slots) are
allocated via `entry_block_alloca` — a helper that hoists `alloca`
instructions to the function's entry block. This lets LLVM combine them
into a single `sub $X, %rsp` at function entry, which is the standard
safe ABI pattern. Mid-function allocas produce dynamic stack adjustment
patterns (`mov %rsp, %r14; mov %rdi, %rsp`) that leak stack across
subsequent calls and cause intermittent segfaults under multi-threaded
test execution.

### ZST handling

Zero-sized types (`()`, empty structs) are handled correctly:
- **Params**: ZST params are elided from the LLVM signature (mirrors rustc).
- **Args**: ZST args are skipped at call sites.
- **Fields**: ZST fields are elided from LLVM struct types (via `filter_void_fields`).
- **Array elements**: ZST array elements use `{}` (LLVM empty struct) → `[N x {}]` is valid.
- **Allocas**: ZST allocas use `i8` fallback (1-byte placeholder) because LLVM size-0 allocas produce undef pointers (UB).

### Recursive struct handling

Recursive types (`struct Node { next: *mut Node }`) are handled by using
opaque `ptr` for `Ref`/`RawPtr` to `Adt` — no pointee type recursion.
The pointee's struct layout is resolved only at dereference sites via
`detect_place_storage_type`, breaking the infinite recursion cycle.

### Variadic function detection

Variadicity is detected from the function's signature text (not a hardcoded
name list). `signature_is_variadic(sig)` checks if the signature contains
`...` inside parens. The `variadic_fns: HashSet<String>` field is populated
by `emit_declare` from the signature text.

### TextEmitter IR validation

TextEmitter IR (`--emit-llvm-ir`) is validated by `llvm-as` smoke tests
(34 tests across `stage18_334` + `stage18_335` + `stage18_336` + `stage18_337`).
This catches the entire class of "TextEmitter IR silently invalid" bugs.

---

## Testing

### Test count

- 676 unit tests (lib)
- 3689 integration tests (`tests/all_tests.rs`)
- 4365 total (100% pass rate single-thread, 0 skipped)

### Running tests

```bash
# Single-thread (deterministic, ulimit -s unlimited required for LLVM)
ulimit -s unlimited
cargo test --release --features llvm-backend -- --test-threads=1

# Multi-thread stress test (auto-tunes threads based on system resources)
bash scripts/run_tests.sh

# Run a specific test module
cargo test --release --features llvm-backend -- stage18_337_recursive_struct_tests
```

### Why `ulimit -s unlimited`?

LLVM 22's recursive optimization passes (CodeGenPrepare, LowerFormalArguments,
Prologepilog) need more than the default 8MB stack on some Linux systems.
Without raising the limit, `landin-stage0` may intermittently segfault inside
`libLLVM.so.22.1` during `--emit-obj` (verified: 0/100 segfaults at
`ulimit -s unlimited` vs ~2% segfault rate at default 8MB).

`scripts/run_tests.sh` handles this automatically.

### §14.5 D1-D8 Deep Review (v0.4 sign-off)

| Dimension | Status | Details |
|-----------|--------|---------|
| D1 Architecture | ✅ | 176 files, 81,573 LOC, no circular deps, all LOC TDs resolved |
| D2 Tech Debt | ✅ | All P0/P1/P2 resolved (114 TDs). Only BLOCKED: TD-INTRINSIC-OVERUSE Phase 2-B/C (needs v0.4+ lang features) |
| D3 Test Coverage | ✅ | 4365 tests (676 lib + 3689 integration), 1:3+ pos:neg ratio |
| D4 Next Stage Readiness | ✅ | v0.4 release-ready, all features complete |
| D5 Design Soundness | ✅ | sret+byval explicit, ZST elision, recursive struct cycle break, TextEmitter IR validated |
| D6 Performance | ✅ | ~30s build, ~24s test single-thread, no O(n²) known |
| D7 Documentation | ✅ | 8 plan docs for Stage 18.3xx + tech-debt-register + process doc v7.3 |
| D8 Pipeline Coverage | ✅ | All 10 expression contexts verified closed (lex→parse→hir→typeck→borrowck→mir→codegen) |

---

## Project Layout

```
landin/
├── src/                          # Compiler source (~82K LOC)
│   ├── bin/                      # CLI entry points
│   │   ├── main.rs               # landin-stage0 (single-file compiler)
│   │   └── landinc.rs            # landinc (multi-file project tool)
│   ├── lexer/                    # Tokenizer
│   ├── parser/                   # AST construction
│   ├── hir/                      # High-level IR
│   ├── typeck/                   # Type checker + borrow checker
│   ├── mir/                      # Mid-level IR (lowering + optimization)
│   │   ├── lower/                # MIR lowering from HIR
│   │   ├── monomorphize/         # Generic instantiation
│   │   ├── substitute.rs         # Type parameter substitution
│   │   └── ...
│   ├── codegen/                  # LLVM IR emission
│   │   ├── llvm/                 # LLVMSysEmitter (production)
│   │   ├── text/                 # TextEmitter (debug)
│   │   ├── emitter/              # Emitter trait + EmitType
│   │   ├── mir_translation/      # MIR → EmitType translation
│   │   ├── trait_dispatch/       # Vtable construction
│   │   └── ...
│   ├── borrowck/                 # Borrow checker
│   ├── driver/                   # Compilation pipeline
│   ├── stdlib/                   # Landin prelude (String/Vec/Box/...)
│   ├── session/                  # Compiler session + diagnostics
│   └── diagnostics/              # Error formatting
├── tests/                        # Integration tests
│   ├── all_tests.rs              # Unified test entry
│   ├── common/mod.rs             # Shared test helpers
│   ├── v0/stage{0..18}/plan/     # Per-stage test files
│   └── fuzz/                     # Fuzz harness
├── docs/                         # Documentation
│   ├── stage-committee-process.md  # SOP (v7.3, 3068 LOC)
│   ├── develop/v0/               # Per-stage dev logs + plans
│   ├── lang-design/              # Language design docs
│   ├── graph/                    # Pipeline graphs + matrix
│   ├── build-guide.md            # Build guide
│   └── testing-guide.md          # Testing guide
├── scripts/                      # Build/test scripts
│   ├── env.sh                     # LLVM env setup
│   ├── setup-llvm-env.sh         # Auto-detect/install LLVM
│   └── run_tests.sh             # Test runner (auto-tunes threads + ulimit)
├── tools/                        # Debug/migration tools
├── examples/                     # Example programs
├── benchmark/                    # Benchmarks
├── rustfmt.toml
└── Cargo.toml
```

---

## Documentation

- **Build guide**: `docs/build-guide.md`
- **Testing guide**: `docs/testing-guide.md`
- **SOP**: `docs/stage-committee-process.md` (v7.3, 3068 LOC)
- **Tech debt register**: `docs/develop/v0/tech-debt-register.md`
- **Per-stage dev logs**: `docs/develop/v0/stage-N/`
- **Per-stage plans**: `docs/develop/v0/stage-N/plan-N.M.md`
- **Language design**: `docs/lang-design/` (07-codegen.md, 25-drop-elaboration.md, etc.)

---

## Roadmap

### v0.4 (current — release-signed-off)

- ✅ All P0/P1/P2 tech-debts resolved
- ✅ §14.5 D1-D8 deep review PASSED
- ✅ System V ABI: sret + byval explicitly emitted
- ✅ ZST handling: params elided, fields filtered, arrays valid
- ✅ Recursive struct: opaque ptr cycle break
- ✅ TextEmitter IR validated by `llvm-as`
- ✅ Variadic function detection from signature
- ✅ Typeck catches ZST return + struct return + trait self + int width mismatches
- ✅ 4365 tests, 0 failures

### v0.4+ (next)

- TD-GENERIC-STRUCT-FIELD-TYS-INFERENCE (P2): MIR-lower type inference improvement for generic struct field_tys substitution (needs dest_local Adt type propagation)
- TD-INTRINSIC-OVERUSE Phase 2-B/C — needs lang features:
  - Primitive type impl (`impl str { fn len(&self) -> usize { ... } }`)
  - Fat pointer construction (`&str` → `(ptr, len)`)
  - `extern "C"` in prelude impl
- `core::fmt` infrastructure (Display/Debug/Formatter/Write traits)
- Cross-compilation to Windows/macOS

---

## Contributing

### Development workflow

Per `docs/stage-committee-process.md` (v7.3):

1. **Self-check (§1.2.1)**: classify task as L1/L2/L3
2. **Design alignment (§13.1)**: read `docs/lang-design/` + `docs/graph/`
3. **MUV (§4)**: smallest verifiable unit of work
4. **Inner review (§5)**: P0/P1 cleanup loop
5. **Acceptance (§3.2)**: `cargo fmt + check + clippy + test --release` all green
6. **Documentation (§8)**: worklog + tech-debt-register + plan doc
7. **Packaging (§19)**: `landin-stage0-v<X>.<Y>.<Z>-stage<N>.<M>-<desc>-r<R>.tar.gz`

### Principles

- §1.0 原則 4: 报错 > 静默 (errors > silent)
- §1.0 原則 6: 通解 > 特解 (general > special-case)
- §1.0 原則 9: 正确 > 妥协 (correct > compromise)
- §2.2: 根因思维 (root-cause thinking)
- §12: 最优 > 最小 (optimal > minimal)
- §20: 迭代审计 (iterative audit — "finding one bug means there are many similar bugs")
- 知识搜索 > 猜测 (knowledge search > guessing)

### License

MIT — see [LICENSE](LICENSE).
