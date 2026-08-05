# Landin

**Author**: redskaber
**Version**: v0.260.0
**Date**: 2026-08-05
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
- **Associated types**: `type Item;` in traits, resolved in impl blocks
- **Object safety**: `dyn Trait` checked against 5 RFC #255 rules
- **Where clauses**: `fn f<T>() where T: Clone + Debug` (trait existence checked)

### Release History

| Version | Status | Key Deliverables |
|---------|--------|-----------------|
| v0.2 | FINAL | NLL borrow checker, `impl Drop`, `dyn Trait`, stdlib MVP |
| v0.3 | RELEASE SIGNED OFF | Sound Copy detection, Task 3 TraitResolver, Task 10 Closure Redesign, Codegen refactoring |
| Task 11 | COMPLETE (Stages 16.49-16.62) | Monomorphization: substs propagation, substitution, collection, naming, per-mono layouts, codegen integration |
| Task 14 | COMPLETE (Stages 16.64-16.65) | Object safety: 5 rules checker + driver integration |
| Task 17 | COMPLETE (Stages 16.67-16.69) | Associated types: MIR Projection + projection_resolver + driver integration |
| Where clauses | COMPLETE (Stage 16.73) | Where clause checking: trait existence verification |
| Deep Review | 10 rounds (all GO) | Stages 16.09, 16.12, 16.15, 16.18, 16.25, 16.33, 16.39, 16.43, 16.59, 16.71 |

### v0.3+ Achievements

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
6. **Task 14: Object Safety** — 5 rules per RFC #255 + driver integration
7. **Task 17: Associated Types** — MIR `TyKind::Projection` + `projection_resolver` + driver
8. **Where Clauses** — Trait existence verification in `check_where_clauses`

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
cargo test --features llvm-backend --lib          # 358 lib tests
cargo test --features llvm-backend --test all_tests  # 2529 integration tests
python3 tests/conformance/run_all.py              # 5224 conformance tests

# Run a Landin program
cargo run --features llvm-backend -- --run examples/hello.lin
```

### Test Statistics (v0.260.0)

| Test Type | Count | Pass Rate |
|-----------|-------|-----------|
| Lib tests | 358 | 100% |
| Integration tests | 2529 | 100% |
| Conformance tests | 5224 | 100% |
| **Total** | **8111** | **100%** |

### Quality Metrics

| Metric | Value |
|--------|-------|
| Source lines | 54,817 |
| Test lines | 46,604 |
| Total lines | 101,421 |
| Source files | 107 |
| Test files | 207 |
| Clippy warnings | 0 |
| Dead code annotations | 2 (documented, intentional) |
| TODO/FIXME in src/ | 0 |
| Deep review rounds | 10 (all GO) |

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
│   ├── ty.rs           — Type system (incl. TyKind::Projection)
│   ├── dyn_trait.rs    — Dyn trait fat-pointer MIR
│   └── drop_elaboration.rs — Drop glue
├── typeck/             — Type checking (checker, unify, tables,
│   │                     projection_resolver, where_clause)
├── borrowck/           — Borrow checking (NLL + region inference)
├── codegen/            — LLVM IR generation
│   ├── emitter.rs      — Emitter trait
│   ├── mir_translation.rs — MIR → EmitType translation (incl. _and_mono)
│   ├── text/           — TextEmitter (text backend)
│   ├── llvm/           — LLVMSysEmitter (LLVM C-API backend)
│   └── trait_dispatch/ — Vtable + dynptr orchestration
├── traits/             — Trait resolver (incl. object_safety)
├── stdlib/             — Standard library
├── resolve/            — Name resolution
├── driver.rs           — Compilation orchestration
├── diagnostics/        — Error reporting
└── session/            — Span + session info
```

### Type System Features

```
Generics (Task 11):
  Parser → AST → HIR → MIR substs propagation → substitute →
  collect_mono_items → build_mono_layouts → codegen integration

Object Safety (Task 14):
  check_trait_object_safety → 5 rules (SelfReturn, SelfInArg,
  GenericMethod, NoReceiver, ByValueReceiver) → driver hook

Associated Types (Task 17):
  AST HirAssocType → MIR TyKind::Projection →
  projection_resolver → resolve_projections_in_mir → driver hook

Where Clauses (Stage 16.73):
  Parser → AST → HIR HirWherePredicate →
  check_where_clauses → trait existence verification → driver hook
```

---

## Documentation

### Key Documents

| Document | Description |
|----------|-------------|
| `docs/stage-committee-process.md` | Development process and quality control (v3.24) |
| `docs/develop/v0/v0.3-complete-design.md` | v0.3 complete design (final state) |
| `docs/develop/v0/v0.4-roadmap.md` | v0.4 roadmap with completion status |
| `docs/develop/v0/task-11-monomorphization-design.md` | Task 11 monomorphization design |
| `docs/develop/v0/api-naming-standard.md` | API naming conventions |

### Pipeline Diagrams

| Document | Description |
|----------|-------------|
| `docs/graph/codegen/architecture.md` | Codegen module architecture |
| `docs/graph/codegen/data-flow.md` | Unified pipeline data flow |
| `docs/graph/pipeline/overview.md` | End-to-end compiler pipeline |
| `docs/graph/type-system/data-flow.md` | Type system + monomorphization data flow |
| `docs/graph/trait-system/data-flow.md` | Trait system data flow |
| `docs/graph/closure/data-flow.md` | Closure data flow |
| `docs/llvm/backend-architecture.md` | LLVM backend architecture |

### Stage 16 Statistics

| Metric | Value |
|--------|-------|
| Stage 16 design docs | 82 |
| Stage 16 test files | 43 |
| Deep review reports | 10 (Round 1-10, all GO) |
| Graph diagrams | 11 |
| LLVM docs | 21 |
| Total docs | 1,060 |

---

## Repository

[https://github.com/redskaber/landin-lang](https://github.com/redskaber/landin-lang)
