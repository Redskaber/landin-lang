# Landin

**Author**: redskaber  
**Version**: v0.361.0  
**License**: MIT  
**Status**: v0.1 stable — pipeline audit-clean

A work-in-progress systems programming language inspired by Rust, using LLVM 19
for code generation. The compiler is written in Rust (~50,000 LOC) and targets
x86_64 and AArch64 Linux.

---

## Quick Start

```bash
# Setup LLVM 19 environment
bash scripts/setup-llvm-env.sh
bash scripts/switch-llvm-version.sh 19

# Build
cargo build --features llvm-backend

# Compile a program
echo 'fn main() { println!("hello world"); 0 }' > hello.lin
./target/debug/landin-stage0 --run hello.lin

# Cross-compile to AArch64
./target/debug/landin-stage0 --emit-obj --target aarch64-unknown-linux-gnu hello.lin
```

---

## Language Features

### Supported (v0.1)

- **Types**: i32/i64/u8/u32/u64/f32/f64/bool/char/str, tuples, arrays, slices
- **ADTs**: struct (named/tuple/unit), enum (with data)
- **Functions**: generic functions, closures, `extern "C"`
- **Traits**: trait definitions, impl blocks, `dyn Trait` (fat pointer dispatch)
- **Pattern matching**: let bindings, match arms, nested destructuring
- **Ownership**: move semantics, `&`/`&mut` borrows, NLL (non-lexical lifetimes)
- **Macros**: `macro_rules!` with 9 fragment specifiers, repetition, hygiene
- **GATs**: Generic Associated Types (Phase 1-3: parsing + projection resolution)
- **Cross-compilation**: `--target` flag for x86_64/aarch64 Linux

### Type Checking (Stage 18.71-18.73)

- Type mismatch in `let` bindings, function returns, if-branches, match arms
- Trait impl signature validation (arg count, types, return type)
- Struct field count validation (missing/extra/unknown/duplicate)
- Tuple index bounds checking
- Pattern arity validation
- Array index type checking
- Assignment target validation
- Cast type validation
- Missing `fn main()` detection
- Associated const completeness checking

### Error System

All 8 error types have structured `Kind` enums + `Span` + `ErrorCode`:

| Error Type | Code | Kind Variants |
|-----------|------|---------------|
| LexError | E001 | 8 |
| ParseError | E100 | 8 |
| LowerError | E200 | 4 |
| ResolveError | E300 | 8 |
| TypeError | E400 | 6 |
| BorrowError | E500 | 9 |
| TraitError | E600 | — |
| CodegenError | E700 | 5 |
| MacroError | E800 | 5 |

---

## Testing

| Category | Count | Description |
|----------|-------|-------------|
| Rust lib tests | 638 | Unit tests in `src/` |
| Integration tests | 2,648 | Tests in `tests/v0/` |
| Conformance tests | 2,935 | `.lin` files in `tests/conformance/` |
| Fuzz/stress tests | 7 | Random + malformed input, large programs |
| **Total** | **6,221** | **0 failures** |

```bash
# Run all tests
cargo test --features llvm-backend
python3 tests/conformance/run_all.py
```

### Test Types

| Type | Status |
|------|--------|
| Functional correctness | ✅ Strong (3,959+ positive) |
| Language standard compliance | ✅ 804 Stage 0 limitation tests |
| Diagnostic quality | ✅ 553 specific ERROR_PATTERNs |
| Robustness/stress | ✅ 7 fuzz + 8 stability tests |
| Cross-compilation | ✅ x86_64 + AArch64 |
| Performance/benchmark | ⚠️ Minimal (5 Instant-based) |

---

## Architecture

```
Source → Lexer → macro_expand → Parser → HIR → Resolve → MIR → Typeck → Borrowck → Codegen
```

- **Lexer**: 4 sub-modules (ident, number, string, operators)
- **Parser**: 7 sub-modules (expr, items, pat, ty, path, stmt, macro_expand)
- **HIR**: 8 lowering sub-modules, `HirCrate` with `DefId`-keyed owners
- **Resolve**: Name resolution + module tree + use imports
- **MIR**: 9 lowering sub-modules, `MirBody` with basic blocks
- **Typeck**: `TypeChecker` + `UnificationTable` + `projection_resolver`
- **Borrowck**: `BorrowChecker` + `region_inference` (NLL)
- **Codegen**: `TextEmitter` (LLVM IR text) + `LLVMSysEmitter` (LLVM C API)

### Stage Isolation (§11)

Each stage receives data (not HIR references) from upstream:
- Typeck receives `FieldTyTable` + `FnSigTable` (pre-computed by driver)
- Codegen receives `CompileResult` (MIR + metadata, zero HIR access)

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
│   ├── mir/           # MIR lowering (9 sub-modules)
│   ├── typeck/        # Type checker (checker, unify, predicates, etc.)
│   ├── borrowck/      # Borrow checker (NLL, region inference)
│   ├── traits/        # Trait resolver (coherence, vtable, dyn Trait)
│   ├── codegen/       # Code generation (text + LLVM backends)
│   ├── diagnostics/   # Error display (DiagnosticBuilder, DiagnosticBuffer)
│   ├── session/       # Session (Span, SourceMap)
│   ├── stdlib/        # Standard library facade
│   └── driver.rs      # Compilation pipeline orchestration
├── tests/
│   ├── v0/            # Integration tests (by stage)
│   ├── conformance/   # .lin conformance suite
│   └── fuzz/          # Fuzz/stress tests
├── docs/              # Design docs, stage plans, gate reviews
├── scripts/           # LLVM setup, version switching
└── Cargo.toml
```

---

## Current Limitations (v0.1)

### Stage 0 Known Limitations

- **Param unify**: Generic type parameters unify with any type (unsound — requires v0.2 monomorphization)
- **Deref on non-Ref**: Pattern bindings on `&self` don't propagate reference types (deferred to v0.2)
- **MIR optimization**: DCE + const_prop implemented but not wired into driver (v0.2 decision)
- **Single-file compilation**: No project/crate system (v0.2 mini-cargo)
- **No incremental compilation**: Deferred to v0.2 (requires project system + monomorphization)

### Unsupported Features

- Cross-compilation to Windows/macOS (Linux only)
- Process macros
- async/await (syntax supported, no runtime)
- Full standard library (String/Vec/Option/Result are facades)
- Self-hosting (far future)

---

## v0.2 Roadmap (Planned)

| Priority | Task | Description |
|----------|------|-------------|
| P0 | Monomorphization | Fix Param unify, enable GAT Phase 4 |
| P0 | Project system (mini-cargo) | Multi-file compilation, crate graph |
| P1 | Full standard library | String, Vec, Option, Result, HashMap |
| P1 | MIR optimization wiring | DCE + const_prop in driver pipeline |
| P2 | Incremental compilation | Dependency graph + MIR hash + cache |
| P2 | Criterion benchmarks | Statistical performance baselines |
| P3 | Self-hosting Phase 0 | Standard library in Landin |

---

## Documentation

- `docs/stage-committee-process.md` — Development process and quality standards
- `docs/lang-design/` — Language design documents (00-19)
- `docs/develop/v0/stage-18/` — Stage design docs and gate reviews
- `docs/tests/` — Test matrix and coverage docs
- `RELEASE_NOTES.md` — Version history
- `docs/build-guide.md` — Build instructions
- `docs/testing-guide.md` — Testing guide

---

## License

MIT — see [LICENSE](LICENSE)
