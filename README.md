# Landin

**Author**: redskaber
**Version**: v0.257.0
**Date**: 2026-08-04
**License**: MIT

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance. The compiler is written in Rust and uses LLVM 19
for code generation via the `llvm-sys` crate.

---

## What is Landin?

Landin is a systems programming language that brings Rust's safety guarantees
to a simpler, more approachable syntax. The compiler is self-hosted (Stage 0
bootstrap) and uses LLVM 19 for code generation.

### Key Language Features

- **Memory safety without GC**: Ownership, borrowing, and lifetimes
- **Zero-cost abstractions**: Closures, traits, generics (monomorphization)
- **Type inference**: No type annotations needed for most locals
- **Pattern matching**: `match` with destructuring
- **Trait system**: Static + dynamic dispatch (`dyn Trait`)
- **Closures**: Capture by value or reference, nested closures
- **Drop semantics**: RAII with `impl Drop`
- **Error reporting**: Span-accurate diagnostics with color support
- **Generic types**: `struct Box<T>`, `enum Opt<T>`, `impl<T> Foo for Bar<T>`

### Release History

| Version | Status | Key Deliverables |
|---------|--------|-----------------|
| v0.2 | FINAL | NLL borrow checker, `impl Drop`, `dyn Trait`, stdlib MVP |
| v0.3 | RELEASE SIGNED OFF | Sound Copy detection, Task 3 TraitResolver, Task 10 Closure Redesign, Codegen refactoring |
| Task 11 | COMPLETE (Stages 16.49-16.62) | Monomorphization: substs propagation, substitution, collection, naming, per-mono layouts, codegen integration |

### v0.3 + Task 11 Achievements

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
5. **Task 11: Monomorphization** — 9 stages, all complete:
   - Phase 1 (1a-1c): Substs propagation (generics_of, TyKind::Adt, AggregateKind::Adt)
   - Phase 2: Type substitution (`substitute(ty, substs)`)
   - Phase 3: Monomorphization collection (`collect_mono_items`)
   - Phase 4 (4a-4c): Specialized naming, per-mono layouts, codegen integration
   - Runtime verified: `Box<i32>`, `Pair<A,B>`, `Opt<T>` all run correctly

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
cargo test --features llvm-backend --lib          # 343 lib tests
cargo test --features llvm-backend --test all_tests  # 2514 integration tests
python3 tests/conformance/run_all.py              # 5224 conformance tests

# Run a Landin program
cargo run --features llvm-backend -- --run examples/hello.lin
```

### Test Statistics (v0.257.0)

| Test Type | Count | Pass Rate |
|-----------|-------|-----------|
| Lib tests | 343 | 100% |
| Integration tests | 2514 | 100% |
| Conformance tests | 5224 | 100% |
| **Total** | **8081** | **100%** |

### Quality Metrics

| Metric | Value |
|--------|-------|
| Source lines | 53,589 |
| Test lines | 46,394 |
| Total lines | 99,983 |
| Source files | 104 |
| Test files | 205 |
| Clippy warnings | 0 |
| Dead code annotations | 2 (documented, intentional) |
| TODO/FIXME in src/ | 0 |
| Deep review rounds | 9 (all GO) |

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
  ├─ 5. Build MonoLayoutMap (per-mono layouts for generic types)
  ├─ 6. Main MIR function bodies (codegen_from_mir)
  └─ 7. Synthesized closure function bodies

Backends:
  ├─ TextEmitter  (text/mod.rs)  → LLVM IR text (.ll)
  └─ LLVMSysEmitter (llvm/mod.rs) → LLVM module → object (.o)
```

### Module Structure

```
src/
├── lexer/              — Tokenization
├── parser/             — AST construction
├── hir/                — HIR lowering + name resolution + generics
├── mir/                — MIR lowering + typeck + borrowck
│   ├── lower/          — HIR → MIR (adt_layout, closure_capture, control_flow,
│   │                     expr_operand, field_resolution, overflow_assert,
│   │                     pattern_bindings)
│   ├── monomorphize/   — Monomorphization (item, mangle, layout)
│   ├── substitute.rs   — Type substitution
│   ├── ty.rs           — Type system
│   ├── dyn_trait.rs    — Dyn trait fat-pointer MIR
│   └── drop_elaboration.rs — Drop glue
├── typeck/             — Type checking (checker, unify, tables)
├── borrowck/           — Borrow checking (NLL + region inference)
├── codegen/            — LLVM IR generation
│   ├── emitter.rs      — Emitter trait
│   ├── mir_translation.rs — MIR → EmitType translation
│   ├── text/           — TextEmitter (text backend)
│   ├── llvm/           — LLVMSysEmitter (LLVM C-API backend)
│   └── trait_dispatch/ — Vtable + dynptr orchestration
├── traits/             — Trait resolver
├── stdlib/             — Standard library
├── resolve/            — Name resolution
├── driver.rs           — Compilation orchestration
├── diagnostics/        — Error reporting
└── session/            — Span + session info
```

### Monomorphization Pipeline (Task 11)

```
Generic type annotation (HIR)
    │
    ├─ Phase 1a: generics_of query (DefId → Vec<ParamTy>)
    ├─ Phase 1b: lower_path_generic_args → SubstsRef in TyKind::Adt
    ├─ Phase 1c: AggregateKind::Adt carries substs
    │
    ├─ Phase 2: substitute(ty, substs) — replace Param with concrete types
    │           ↓ integrated into field_resolution
    │
    ├─ Phase 3: collect_mono_items — walk MIR, dedup (def_id, substs)
    │           ↓ HashSet<MonoItem>
    │
    ├─ Phase 4a: mangle_ty / mono_item_name — specialized LLVM names
    ├─ Phase 4b: build_mono_layouts — per-mono AdtLayout with substituted fields
    │             ↓ MonoLayoutMap (HashMap<MonoLayoutKey, AdtLayout>)
    │
    └─ Phase 4c: mir_type_to_emit_type_with_layouts_and_mono
                  ↓ lookup_mono_layout first, fall back to AdtLayouts
                  ↓ threaded through entire codegen pipeline
```

---

## Documentation

### Key Documents

| Document | Description |
|----------|-------------|
| `docs/stage-committee-process.md` | Development process and quality control (v3.24) |
| `docs/develop/v0/v0.3-complete-design.md` | v0.3 complete design (final state) |
| `docs/develop/v0/task-11-monomorphization-design.md` | Task 11 monomorphization design |
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
| `docs/graph/type-system/data-flow.md` | Type system + monomorphization data flow |
| `docs/graph/error-system/data-flow.md` | Error system data flow |
| `docs/graph/trait-system/data-flow.md` | Trait system data flow |
| `docs/llvm/backend-architecture.md` | LLVM backend architecture |

### Stage 16 Statistics

| Metric | Value |
|--------|-------|
| Stage 16 design docs | 71 |
| Stage 16 test files | 41 |
| Deep review reports | 9 (Round 1-9, all GO) |
| Graph diagrams | 11 |
| LLVM docs | 21 |
| Total docs | 1,037 |

---

## Repository

[https://github.com/redskaber/landin-lang](https://github.com/redskaber/landin-lang)
