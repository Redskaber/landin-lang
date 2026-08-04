# Landin

**Author**: redskaber
**Version**: v0.237.0 (v0.2 FINAL + v0.3 RELEASE SIGNED OFF + Task 11 Phase 1b: substs propagation)
**Date**: 2026-08-04

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance. The compiler is written in Rust and uses LLVM as its
backend via the `llvm-sys` crate.

---

## What is Landin?

Landin is a systems programming language that aims to bring Rust's safety
guarantees to a simpler, more approachable syntax. The compiler is
self-hosted (Stage 0 bootstrap) and uses LLVM 19 for code generation.

### Key Language Features

- **Memory safety without GC**: Ownership, borrowing, and lifetimes
- **Zero-cost abstractions**: Closures, traits, generics
- **Type inference**: No type annotations needed for most locals
- **Pattern matching**: `match` with destructuring
- **Trait system**: Static + dynamic dispatch (`dyn Trait`)
- **Closures**: Capture by value or reference, nested closures
- **Drop semantics**: RAII with `impl Drop`
- **Error reporting**: Span-accurate diagnostics with color support

### v0.3 Achievements (RELEASE SIGNED OFF)

1. **Sound Copy detection** — Field-level derivation mirroring Rust's `#[derive(Copy)]`
2. **Task 3: TraitResolver Keys** — DefId-keyed trait impl lookup (type-safe, no Spur)
3. **Task 10: Closure Redesign** — 100% complete:
   - All closures use synthesized `call` function (Strategy A)
   - No-capture, i32/struct/mutable captures, nested up to 4+ levels
   - Typeck + borrowck + codegen all work end-to-end
   - Runtime verified for all closure patterns
4. **Codegen Architecture Refactoring** — Complete:
   - Unified pipeline (`run_codegen_pipeline`) for both backends
   - Text/LLVM backends properly separated
   - Zero dead code, zero unused imports
   - Full documentation (8 graph diagrams + 21 LLVM docs)

### v0.2 Achievements (FINAL)

- Full NLL borrow checker with region inference
- `impl Drop` + RAII (recursive drop, double-drop prevention)
- `dyn Trait` fat-pointer dispatch (vtable + dynptr globals)
- Stdlib MVP (Copy/Clone/Drop, arithmetic traits, IO traits)
- 7700+ tests (100% pass rate)

---

## Build & Test

### Prerequisites

- Rust (stable, via rustup)
- LLVM 19 (build-server or user-installed)
- `cc` / `clang` (for linking)

### Setup

```bash
# Set up LLVM 19 environment
source scripts/setup-llvm-env.sh

# Build
cargo build --features llvm-backend

# Run tests
cargo test --features llvm-backend --lib          # 244 lib tests
cargo test --features llvm-backend --test all_tests  # 2414 integration tests
python3 tests/conformance/run_all.py              # 5224 conformance tests

# Run a Landin program
cargo run --features llvm-backend -- --run examples/hello.lin
```

### Test Statistics (v0.235.0)

| Test Type | Count | Pass Rate |
|-----------|-------|-----------|
| Lib tests | 244 | 100% |
| Integration tests | 2414 | 100% |
| Conformance tests | 5224 | 100% |
| **Total** | **7882** | **100%** |

---

## Architecture

### Compiler Pipeline

```
Source (.lin) → Lexer → Parser → HIR → MIR → Typeck → Borrowck → Codegen → LLVM IR → Object → Executable
```

### Codegen Architecture

```
run_codegen_pipeline (unified, shared by all backends)
  ├─ 1. Module header + panic declarations
  ├─ 2. Vtable globals (before function bodies)
  ├─ 3. Dyn trait fat-pointer globals
  ├─ 4. Drop glue functions
  ├─ 5. Main MIR function bodies (codegen_from_mir)
  └─ 6. Synthesized closure function bodies

Backends:
  ├─ TextEmitter  (text/mod.rs)  → LLVM IR text (.ll)
  └─ LLVMSysEmitter (llvm/mod.rs) → LLVM module → object (.o)
```

### Module Structure

```
src/
├── lexer/          — Tokenization
├── parser/         — AST construction
├── hir/            — HIR lowering + name resolution
├── mir/            — MIR lowering + typeck + borrowck
│   ├── lower/      — HIR → MIR
│   ├── ty/         — Type system
│   └── drop_elaboration/ — Drop glue
├── typeck/         — Type checking
├── borrowck/       — Borrow checking (NLL + region inference)
├── codegen/        — LLVM IR generation
│   ├── emitter.rs  — Emitter trait
│   ├── text/       — TextEmitter (text backend)
│   ├── llvm/       — LLVMSysEmitter (LLVM C-API backend)
│   └── trait_dispatch/ — Vtable + dynptr orchestration
├── traits/         — Trait resolver
├── stdlib/         — Standard library
├── driver.rs       — Compilation orchestration
└── diagnostics/    — Error reporting
```

---

## Documentation

### Key Documents

| Document | Description |
|----------|-------------|
| `docs/stage-committee-process.md` | Development process and quality control |
| `docs/develop/v0/v0.3-complete-design.md` | v0.3 complete design (final state) |
| `docs/develop/v0/task-10-closure-redesign-design.md` | Closure redesign design |
| `docs/develop/v0/task-3-traitresolver-keys-design.md` | TraitResolver keys design |
| `docs/develop/v0/api-naming-standard.md` | API naming conventions |

### Pipeline Diagrams

| Document | Description |
|----------|-------------|
| `docs/graph/codegen/architecture.md` | Codegen module architecture |
| `docs/graph/codegen/emitter-trait.md` | Emitter trait hierarchy |
| `docs/graph/codegen/data-flow.md` | Unified pipeline data flow |
| `docs/graph/codegen/backend-comparison.md` | Text vs LLVM backend |
| `docs/graph/pipeline/overview.md` | End-to-end compiler pipeline |
| `docs/graph/closure/data-flow.md` | Closure data flow |
| `docs/llvm/backend-architecture.md` | LLVM backend architecture |

### Stage 16 Statistics

| Metric | Value |
|--------|-------|
| Stage 16 design docs | 54 |
| Stage 16 test files | 31 |
| Stage 16 tests | 256 |
| Deep review reports | 8 (Round 1-8, all GO) |
| Graph diagrams | 8 |
| LLVM docs | 21 |

---

## License

MIT

## Repository

[https://github.com/redskaber/landin-lang](https://github.com/redskaber/landin-lang)
