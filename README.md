# Landin

> **Author**: redskaber  
> **Version**: v0.385.0 (Stage 18.117 — Span::DUMMY cleanup: checker.rs infer_rvalue + remaining Ty::from_kind conversions)  
> **License**: MIT  
> **Status**: v0.1 stable, v0.2 P0 monomorphization COMPLETE (S2-S11 all fixed)

A work-in-progress systems programming language inspired by Rust, using LLVM 19
for code generation. The compiler is written in Rust (~50,000 LOC) and targets
x86_64 and AArch64 Linux.

---

## Quick Start

```bash
# 1. Setup LLVM 19 environment (auto-detects or installs LLVM 19 .deb packages)
source scripts/setup-llvm-env.sh
bash scripts/switch-llvm-version.sh 19

# 2. Build
cargo build --features llvm-backend

# 3. Compile and run a program
echo 'fn main() { println!("hello world"); 0 }' > hello.lin
./target/debug/landin-stage0 --run hello.lin

# 4. Cross-compile to AArch64
./target/debug/landin-stage0 --emit-obj --target aarch64-unknown-linux-gnu hello.lin
```

### CLI Flags

| Flag | Description |
|------|-------------|
| `--compile` | Full pipeline (lex → parse → typeck → borrowck → codegen) |
| `--emit-ast` | Stop after parse, print AST |
| `--emit-llvm-ir` | Emit LLVM IR text (.ll) |
| `--emit-obj` | Emit object file (.o) via LLVM TargetMachine |
| `--emit-bin` | Emit executable (link with `cc`) |
| `--run` | Compile, link, execute, print exit code |
| `--target <triple>` | Cross-compile target (x86_64/aarch64-unknown-linux-gnu) |
| `--color <auto\|always\|never>` | Diagnostic color output mode |

---

## Language Features

### Supported (v0.364)

| Category | Features |
|----------|----------|
| **Primitive types** | i8/i16/i32/i64/i128/isize, u8/u16/u32/u64/u128/usize, f32/f64, bool, char, str |
| **Composite types** | Tuples (all arities), arrays `[T; N]`, slices `&[T]`, references `&T`/`&mut T`, raw pointers `*const T`/`*mut T` |
| **ADTs** | Struct (named/tuple/unit), Enum (with data, match, discriminant) |
| **Functions** | Generic functions, `extern "C"`, `unsafe`, variadic |
| **Closures** | `\|...\|`, `move \|...\|`, capture by ref/mut |
| **Traits** | Definition, impl, supertraits, default methods, `dyn Trait` (fat pointer dispatch), associated types, GATs (Phase 1-3) |
| **Pattern matching** | `let`, `match`, nested, struct, tuple, or-patterns, destructuring |
| **Ownership** | Move semantics, use-after-move detection, `&`/`&mut` borrows, NLL (non-lexical lifetimes), double-mut detection |
| **Macros** | `macro_rules!` with 9 fragment specifiers, repetition, hygiene; built-in `println!`/`print!`/`eprintln!`/`eprint!` |
| **Cross-compilation** | `--target x86_64-unknown-linux-gnu` / `aarch64-unknown-linux-gnu` |
| **MIR optimization** | DCE (dead code elimination) + const_prop (constant propagation + folding) — wired at Stage 18.96 |

### Type Checking

- Type mismatch in `let` bindings, function returns, if-branches, match arms
- Trait impl signature validation (arg count, types, return type)
- Struct field count validation (missing/extra/unknown/duplicate)
- Tuple index bounds, pattern arity, array index type, assignment target, cast type
- Missing `fn main()` detection, associated const completeness

### Error System

All 8 error types have structured `Kind` enums + `Span` + `ErrorCode`:

| Error Type | Code | Kind Variants |
|-----------|------|---------------|
| LexError | E001 | 7 |
| ParseError | E100 | 7 |
| LowerError | E200 | 4 |
| ResolveError | E300 | 8 |
| TypeError | E400 | 6 |
| BorrowError | E500 | 9 |
| TraitError | E600 | — |
| CodegenError | E700 | 5 |
| MacroError | E800 | 5 |

All errors carry a 9-field `CompileErrors` struct with diagnostic display (source snippets + color).

---

## Testing

| Category | Count | Description |
|----------|-------|-------------|
| Rust lib tests | 640 | Unit tests in `src/` |
| Integration tests | 2,613 | Tests in `tests/v0/` (excl. 35 OOM-killed runtime tests) |
| Conformance tests | 2,935 | `.lin` files in `tests/conformance/` |
| Fuzz/stress tests | 7 | Random + malformed input, large programs |
| **Total** | **6,195** | **0 failures** (runtime tests skipped due to 4GB RAM OOM) |

```bash
# Run all Rust tests
cargo test --features llvm-backend --lib
cargo test --features llvm-backend --tests -- --test-threads=2

# Run conformance suite (2935 .lin files)
python3 tests/conformance/run_all.py
```

---

## Architecture

### Compilation Pipeline

```
Source
  │
  ▼  Lexer ──→ Vec<Token> + Vec<LexError>
  │
  ▼  macro_expand ──→ Vec<Token> (expanded)
  │
  ▼  Parser ──→ Crate<ast::Item> + Vec<ParseError>
  │
  ▼  HIR Lower ──→ HirCrate (DefId-keyed owners + bodies)
  │
  ▼  Resolve ──→ mutates HIR (Res on paths, module tree, use imports)
  │
  ▼  MIR Lower ──→ MirBody (basic blocks + statements + terminator)
  │
  ▼  TypeCheck ──→ mutates MIR (resolved types in local_decls)
  │
  ▼  BorrowCheck ──→ mutates MIR (NLL + region inference + drop elaboration)
  │
  ▼  Writeback ──→ type propagation + closure substitution
  │
  ▼  MIR Optimization ──→ DCE → const_prop → DCE (Stage 18.96)
  │
  ▼  Codegen ──→ LLVM IR (TextEmitter text or LLVMSysEmitter C API)
  │
  ▼  Link ──→ Object file → Executable (via `cc`)
```

### Stage Isolation (§11)

Each stage receives **data** (not HIR references) from upstream:
- Typeck receives `FieldTyTable` + `FnSigTable` (pre-computed by driver)
- Codegen receives `CompileResult` (MIR + metadata, zero HIR access)
- MIR opt receives `&mut MirBody` (after borrowck, before codegen)

### Module Organization

| Module | Sub-modules | Responsibility |
|--------|-------------|----------------|
| `lexer/` | 4 (ident, number, string, reader) | Tokenization |
| `parser/` | 7 (expr, items, pat, ty, path, stmt, macro_expand) | AST construction |
| `hir/` | 8 (lower sub-modules) | AST → HIR lowering |
| `resolve/` | — | Name resolution + module tree |
| `mir/` | 9 (lower sub-modules + optimization + monomorphize) | HIR → MIR + opt |
| `typeck/` | — (checker, unify, predicates, projection_resolver) | Type inference |
| `borrowck/` | — (NLL + region_inference + drop_elaboration) | Ownership/borrow check |
| `traits/` | — (resolver, coherence, vtable, error) | Trait resolution |
| `codegen/` | — (text + llvm backends + dyn_trait_emit) | LLVM IR generation |
| `driver.rs` | — | Pipeline orchestration |

---

## Project Structure

```
landin-stage0/
├── src/
│   ├── ast/           # AST node definitions
│   ├── lexer/         # Lexer (4 sub-modules)
│   ├── parser/        # Parser (7 sub-modules + macro_expand)
│   ├── hir/           # HIR lowering (8 sub-modules)
│   ├── resolve/       # Name resolution
│   ├── mir/           # MIR lowering (9 sub-modules) + optimization
│   ├── typeck/        # Type checker
│   ├── borrowck/      # Borrow checker (NLL, region inference)
│   ├── traits/        # Trait resolver (coherence, vtable, dyn Trait)
│   ├── codegen/       # Code generation (text + LLVM backends)
│   ├── diagnostics/   # Error display (DiagnosticBuilder, DiagnosticBuffer)
│   ├── session/       # Session (Span, SourceMap)
│   ├── stdlib/        # Standard library facade
│   └── driver.rs      # Compilation pipeline orchestration
├── tests/
│   ├── v0/            # Integration tests (by stage)
│   ├── conformance/   # .lin conformance suite (2935 files)
│   └── fuzz/          # Fuzz/stress tests
├── docs/              # Design docs, stage plans, gate reviews
│   ├── stage-committee-process.md  # Development process SOP
│   ├── lang-design/   # Language design (00-19)
│   ├── develop/v0/    # Stage dev logs + plans + gate reviews
│   ├── tests/         # Test matrix + coverage
│   ├── llvm/          # LLVM integration docs
│   └── agent-team/    # Agent roles + collaboration
├── scripts/           # LLVM setup, version switching
├── tools/             # Auxiliary tools
├── benchmark/         # Benchmark programs
├── examples/          # Example .lin programs
├── Cargo.toml
├── RELEASE_NOTES.md
└── README.md
```

---

## Current Limitations (v0.364)

### Type System

- **Param unify unsound**: Generic type parameters unify with any type (requires v0.2 monomorphization)
- **Deref on non-Ref**: Pattern bindings on `&self` don't propagate reference types (v0.2)
- **`LocalId(0)` fallback**: Non-Local borrowed places in region constraints (v0.2 field projection)

### Code Generation

- ~~MIR optimization not wired~~ ✅ Stage 18.96 (DCE → const_prop → DCE)
- **Single-file compilation**: No project/crate system (v0.2 mini-cargo)
- **No incremental compilation**: Full recompile every time (v0.2 — requires project system)
- **BinaryOp2 fallback**: Range expressions in codegen produce "0" with warning (v0.2 CodegenResult)

### Platform Support

- **Linux only**: No Windows/macOS target triples (v0.2 cross-compile expansion)
- **No ABI diversity**: Only `extern "C"` tested (v0.2 `extern "system"`, `extern "Rust"`)

### Standard Library

- **Facade only**: String/Vec/Option/Result are type stubs, not real implementations (v0.2 full stdlib)
- **No `format!`/`write!`**: Only `println!`/`print!`/`eprintln!`/`eprint!` (v0.2 format macros)

### Unsupported Features

- Process macros, async/await runtime (syntax supported, no runtime)
- Self-hosting (far future)

---

## v0.2 Roadmap

| Priority | Task | Status | Description |
|----------|------|--------|-------------|
| **P0** | Monomorphization | Next | Fix Param unify, enable GAT Phase 4, type-specific codegen |
| **P0** | Project system (mini-cargo) | Next | Multi-file compilation, crate graph, dependencies |
| **P1** | Full standard library | Pending | String, Vec, Option, Result, HashMap, Iterator |
| ~~P1~~ | ~~MIR optimization wiring~~ | ✅ Stage 18.96 | DCE + const_prop in driver |
| ~~P1~~ | ~~TraitError location migration~~ | ✅ Stage 18.95 | Moved to `traits/error.rs` |
| **P2** | Incremental compilation | Pending | Dependency graph + MIR hash + cache (requires P0) |
| **P2** | Criterion benchmarks | Pending | Statistical performance baselines |
| **P2** | Windows/macOS targets | Pending | Cross-compilation expansion |
| **P3** | Self-hosting Phase 0 | Future | Standard library in Landin |
| **P3** | Self-hosting Phase 1-5 | Future | Full bootstrap |

---

## Documentation

| Document | Description |
|----------|-------------|
| [`docs/stage-committee-process.md`](docs/stage-committee-process.md) | Development process + quality standards SOP (v5.0) |
| [`docs/lang-design/`](docs/lang-design/) | Language design documents (00-19) |
| [`docs/develop/v0/stage-18/`](docs/develop/v0/stage-18/) | Stage 18 design docs + gate reviews |
| [`docs/develop/v0/v0.1-capability-boundaries.md`](docs/develop/v0/v0.1-capability-boundaries.md) | v0.1 capability boundaries + limitations |
| [`docs/develop/v0/v0.4-roadmap.md`](docs/develop/v0/v0.4-roadmap.md) | v0.4 roadmap design |
| [`docs/develop/v0/v0.5-roadmap.md`](docs/develop/v0/v0.5-roadmap.md) | v0.5 roadmap design |
| [`docs/tests/matrix.md`](docs/tests/matrix.md) | Global test matrix |
| [`docs/tests/pipeline-test-coverage.md`](docs/tests/pipeline-test-coverage.md) | Pipeline test path coverage |
| [`docs/llvm/`](docs/llvm/) | LLVM integration docs |
| [`docs/build-guide.md`](docs/build-guide.md) | Build instructions |
| [`docs/testing-guide.md`](docs/testing-guide.md) | Testing guide |
| [`RELEASE_NOTES.md`](RELEASE_NOTES.md) | Version history (latest: v0.364.0) |

### Recent Stage History

| Stage | Version | Summary |
|-------|---------|---------|
| 18.96 | v0.364.0 | MIR optimization wiring (DCE + const_prop) |
| 18.95 | v0.363.0 | TraitError location migration (driver → traits/error) |
| 18.94 | v0.362.0 | Documentation sync + README rewrite + v0.1 boundaries |
| 18.93 | v0.361.0 | Deep audit v4 + final polish (audit-clean) |
| 18.92 | v0.360.0 | Error type Kind enums (all 8 error types) |

---

## Development Process

This project follows `docs/stage-committee-process.md` (v5.0) — a strict stage-committee SOP with:

- **§3.2 Delivery acceptance**: cargo clean + build + fmt + clippy + test (all must pass)
- **§8 Documentation sync**: Every code change syncs docs (Cargo.toml, README, dev-log, etc.)
- **§10 API naming**: Standardized `<verb>_<noun>` function names, no glob re-exports
- **§11 Interface isolation**: Stages communicate via data contracts, not internal function calls
- **§13.1 Stage start design alignment**: Each stage starts by consulting `docs/lang-design/`
- **§14 Deep review**: 8-dimension audit at stage end (D1-D8)

---

## License

MIT — see [LICENSE](LICENSE)
