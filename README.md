# Landin

> **Author**: redskaber
> **Version**: v0.493.0 (Stage 18.327 — P1 codegen bug 根因修复完成: 10 bugs fixed — opaque pointer migration: GEP/load/store/entry block/typed ptr)
> **License**: MIT
> **Status**: v0.4 stable. 4317 tests, 0 failures. 类 Rust 原始类型扩展模型完成. P1 codegen 根因修复完成.

A work-in-progress systems programming language inspired by Rust, using
LLVM 22 (llvm-sys 221) for code generation. The compiler is written in
Rust (~50,000 LOC) and targets x86_64 + AArch64 Linux.

---

## Quick Start

```bash
# 1. Setup LLVM environment (auto-detects LLVM 22, falls back to LLVM 19)
source scripts/setup-llvm-env.sh
# Or source the env.sh helper (sets PATH + LD_LIBRARY_PATH for LLVM 22):
source scripts/env.sh

# 2. Build
cargo build --features llvm-backend

# 3. Compile and run a program
echo 'fn main() { println!("hello world"); 0 }' > hello.lin
./target/debug/landin-stage0 --run hello.lin

# 4. Cross-compile to AArch64
./target/debug/landin-stage0 --emit-obj --target aarch64-unknown-linux-gnu hello.lin

# 5. Multi-file project (mini-cargo)
landinc new my_project && cd my_project
landinc build --release
landinc run
```

### CLI Flags (`landin-stage0` — single-file compiler)

| Flag | Description |
|------|-------------|
| `--compile` | Full pipeline (lex → parse → typeck → borrowck → codegen) |
| `--emit-llvm-ir` | Emit LLVM IR text (implies --compile) |
| `--emit-obj` | Emit object file `.o` (requires `llvm-backend` feature) |
| `--emit-bin` | Emit executable (requires `llvm-backend`) |
| `--run` | Compile, link, and run (requires `llvm-backend`) |
| `--emit-tokens` | Emit token stream only (debug) |
| `--emit-ast` | Emit AST only (debug) |
| `--color WHEN` | Color output: `auto` / `always` / `never` (default: auto) |
| `--target TRIPLE` | Cross-compile target (e.g. `aarch64-unknown-linux-gnu`) |

### `landinc` Subcommands (multi-file project tool)

| Subcommand | Description |
|------------|-------------|
| `landinc build [--release]` | Compile project (debug or optimized) |
| `landinc run` | Compile + execute (requires `llvm-backend`) |
| `landinc check` | Type-check without codegen |
| `landinc new <name> [--lib]` | Create new project skeleton |
| `landinc clean` | Remove `target/` |

---

## Language Features

### Supported (v0.4)

- **Primitive types**: `i8`–`i128`, `u8`–`u128`, `f32`/`f64`, `bool`, `char`, `str`, `()`, `Never`
- **Composite types**: struct (named/tuple/unit), enum, array `[T; N]`, tuple
- **Ownership & borrowing**: `&T`, `&mut T`, moves, NLL (non-lexical lifetimes)
- **Generics**: `fn foo<T>(x: T)`, `struct Vec<T>`, `impl<T> Vec<T>`
- **Traits**: declaration, `impl Trait for Type`, `dyn Trait` (object-safe only), supertraits
- **Where clauses**: `fn foo<T>() where T: Copy + Clone`
- **Pattern matching**: `match`, `if let`, `let else`, destructuring
- **Closures**: `|x| x + 1`, capturing by ref/move
- **Macros**: `macro_rules!`, built-in `println!`/`print!`/`eprintln!`/`eprint!`/`assert!`/`panic!`/`vec!`/`format!`/`dbg!`/`write!`
- **Stdlib MVP**: `Option<T>`, `Result<T, E>`, `Box<T>`, `Vec<T>`, `String` (with intrinsic methods)
- **FFI**: `extern "C" { fn foo(...); }`, `#[no_mangle]`, `#[link(name = "c")]`
- **Unsafe**: `unsafe fn`, `unsafe impl`, `unsafe block`, raw pointers `*const T` / `*mut T`
- **Codegen**: LLVM 22 backend (text IR + native object via `llvm-sys`)
- **Cross-compile**: `--target aarch64-unknown-linux-gnu`, `--target x86_64-pc-windows-gnu`
- **Diagnostics**: colored error output with source context + error codes (E1xx–E8xx)

### Class Rust Architecture (Stage 18.284-18.297)

- **Primitive type extension**: `impl MyTrait for i32` works (trait impl on primitives)
- **Inherent impl forbidden on primitives**: `impl i32 { fn method {} }` → error E0117-like
- **Inherent impl conflict detection**: duplicate `impl Type { fn same {} }` → error
- **Intrinsic dispatch**: marker body `loop {}` + post-resolution dispatch (类 Rust `#[rustc_intrinsic]`)
- **Coherence checking**: trait + inherent impl conflict detection

---

## Testing

```bash
# Run all Rust unit + integration tests (4203 tests)
cargo test --release --features llvm-backend

# Run lib tests only (676 tests, includes borrowck/typeck/codegen internals)
cargo test --release --lib --features llvm-backend

# Run integration tests only (3527 tests, end-to-end .lin file compilation)
cargo test --release --test all_tests --features llvm-backend

# Run conformance suite (2935 .lin test files)
./tests/conformance/run.sh
```

**Test stats**: 676 lib + 3641 integration = **4317 tests, 0 failures, 0 warnings, 0 clippy issues**.

---

## Architecture

```text
source text
    │
    ▼
1. lexer::tokenize           → tokens + lex errors
    │
    ▼
2. parser::Parser::parse_crate → AST crate + parse errors
    │                        (+ macro_rules! expansion)
    ▼
3. hir::lower::lower_crate   → HIR crate
    │
    ▼
4. resolve::resolve_crate    → mutates HIR (sets Res on paths)
    │
    ▼
5. mir::lower::lower_hir_body_to_mir  (per body)
    │
    ▼
6. typeck::check_mir_body    → mutates MIR (writes resolved types)
    │
    ▼
6.5. mir::drop_elaboration::elaborate_drops  → insert Drop terminators
    │
    ▼
7. borrowck::check_mir_body_with_dataflow  → borrow/move errors (NLL)
    │
    ▼
8. codegen::codegen_crate    → LLVM IR text / LLVM module
    │
    ▼
9. (optional) cc linker      → executable
```

### Module Responsibilities

| Module | Responsibility | Key Files |
|--------|---------------|-----------|
| `lexer` | Tokenization | `lexer/mod.rs` |
| `parser` | AST + macro expansion | `parser/mod.rs`, `parser/macro_expand/`, `parser/builtin_macros/` |
| `hir` | HIR lowering + name resolution | `hir/lower/`, `hir/mod.rs` |
| `resolve` | Path/use resolution | `resolve/mod.rs` |
| `mir` | MIR + drop elaboration + dyn trait | `mir/lower/`, `mir/drop_elaboration.rs`, `mir/dyn_trait.rs` |
| `typeck` | Type checking + unification | `typeck/checker.rs`, `typeck/unify.rs` |
| `borrowck` | Ownership + NLL liveness | `borrowck/mod.rs`, `borrowck/region_inference.rs`, `borrowck/liveness.rs` |
| `codegen` | LLVM IR emission | `codegen/mod.rs`, `codegen/pipeline.rs`, `codegen/runtime.rs`, `codegen/llvm/` |
| `driver` | Pipeline orchestration | `driver/mod.rs`, `driver/compile_inner.rs` |
| `stdlib` | Core/alloc/std type registry + vtable layout | `stdlib/mod.rs`, `stdlib/prelude.rs`, `stdlib/trait_methods.rs`, `stdlib/vtable_layout.rs` |
| `traits` | TraitResolver + coherence + vtable dispatch | `traits/resolver.rs`, `traits/resolver_queries.rs`, `traits/error.rs` |
| `diagnostics` | Error rendering (color, source context) | `diagnostics/mod.rs` |
| `cargo` | Mini-cargo manifest + build orchestration | `cargo.rs` |

---

## Project Structure

```text
landin-stage0/
├── src/
│   ├── ast/              # AST data structures
│   ├── lexer/            # Tokenizer
│   ├── parser/           # Parser + macro_expand/ + builtin_macros/
│   ├── hir/              # HIR + lower/ (AST → HIR)
│   ├── resolve/          # Name resolution
│   ├── mir/              # MIR + lower/ (HIR → MIR) + drop_elaboration + dyn_trait
│   ├── typeck/           # Type checker + unify + where_clause
│   ├── borrowck/          # Borrow checker + region_inference + liveness
│   ├── codegen/          # LLVM IR emission + runtime + llvm/ (llvm-sys wrappers)
│   ├── driver/           # Pipeline orchestration + module_loader
│   ├── stdlib/           # Type registry + prelude + vtable layout
│   ├── traits/           # TraitResolver + coherence + error
│   ├── diagnostics/      # Error rendering
│   ├── session/          # SourceFile + SourceMap + Span
│   ├── bin/
│   │   ├── main.rs       # landin-stage0 (single-file compiler)
│   │   └── landinc.rs    # landinc (multi-file project tool)
│   ├── lib.rs            # Crate root + public API re-exports
│   └── cargo.rs          # Mini-cargo (ProjectManifest + build_project)
├── tests/
│   ├── v0/stage-N/       # Stage-organized test files (3527 integration tests)
│   └── conformance/      # Conformance suite (2935 .lin files)
├── docs/
│   ├── lang-design/      # Language spec (00-18)
│   ├── graph/            # Pipeline + data-flow diagrams
│   ├── develop/v0/       # Stage dev logs + tech-debt-register
│   ├── stage-committee-process.md  # Dev process SOP
│   └── worklog.md        # Stage-by-stage work log
├── scripts/
│   ├── setup-llvm-env.sh # LLVM 22 (or 19 fallback) environment setup
│   ├── switch-llvm-version.sh
│   └── env.sh            # LLVM 22 env helper (PATH + LD_LIBRARY_PATH)
├── examples/             # Example .lin programs
├── Cargo.toml            # llvm-sys 221, optional llvm-backend feature
└── README.md             # This file
```

---

## Current Limitations (v0.493.0, Stage 18.312)

### Type System

- **`LocalId(0)` fallback**: Non-Local borrowed places in region constraints (v0.2+ field projection)
- **Deref on non-Ref**: Pattern bindings on `&self` don't propagate reference types (v0.2+)

### Code Generation

- ~~Single-file compilation~~ ✅ Stage 18.152-18.155 (`landinc` + `compile_project`)
- ~~BinaryOp2 fallback~~ ✅ Stage 18.151 (returns `Err(CodegenError)`)
- ~~MIR optimization not wired~~ ✅ Stage 18.96 (DCE → const_prop → DCE)
- **No incremental compilation**: Full recompile every time (v0.2+ requires project system)

### Standard Library

- **Box/Vec/String**: ✅ Implemented via intrinsics (Stage 18.178-18.231)
- **HashMap/BTreeMap/Rc/Arc/Cell/RefCell**: Name-only placeholders, no implementation (v0.5+)
- **File/Path/TcpStream/Mutex**: Name-only placeholders, no implementation (v0.5+)
- **format!**: ✅ Stage 18.202 (variadic intrinsic, no `core::fmt` infrastructure)
- **Drop trait**: ✅ Stage 18.193 (Box auto-drop via `__landin_dealloc`)

### Platform Support

- **Linux only**: No Windows/macOS target triples (v0.2+ cross-compile expansion)
- **No ABI diversity**: Only `extern "C"` tested (v0.2+ `extern "system"`, `extern "Rust"`)

### v0.5+ Language Features (BLOCKED)

These are required to migrate remaining intrinsics to real prelude impls:

1. **`sizeof(T)`** — generic type size calculation (unlocks `Box::new` + `Vec::push` real body)
2. **Fat pointer ops** — deconstruct + construct (unlocks `String::as_str` real body)
3. **`core::fmt` infrastructure** — Display/Debug/Formatter/Write (unlocks `format!` real body)
4. **Orphan rule** — multi-crate coherence (v0.2+ deferred)

---

## Roadmap

### v0.4 (Current — Stable Release)

| Priority | Task | Status |
|----------|------|--------|
| ✅ | Monomorphization (per-mono codegen) | ✅ Stage 18.103 |
| ✅ | Project system (mini-cargo) | ✅ Stage 18.152-18.155 |
| ✅ | MIR optimization (DCE + const_prop) | ✅ Stage 18.96 |
| ✅ | Box/Vec/String intrinsics | ✅ Stage 18.178-18.231 |
| ✅ | Class Rust primitive impl model | ✅ Stage 18.284-18.297 |
| ✅ | LOC < 1500 for all source files | ✅ Stage 18.305-18.310 |
| ✅ | runtime.rs/prelude.rs cleanup | ✅ Stage 18.311-18.312 |

### v0.5+ (Next Major)

| Priority | Task | Description |
|----------|------|-------------|
| **P0** | `sizeof(T)` | Generic type size calculation |
| **P0** | Fat pointer ops | Deconstruct + construct syntax |
| **P1** | `core::fmt` | Display/Debug/Formatter/Write infrastructure |
| **P1** | HashMap/BTreeMap | Real implementations |
| **P2** | Incremental compilation | Dependency graph + MIR hash + cache |
| **P2** | Windows/macOS targets | Cross-compilation expansion |
| **P3** | Orphan rule | Multi-crate coherence |
| **P3** | Self-hosting Phase 0 | Standard library in Landin |

---

## Documentation

| Document | Description |
|----------|-------------|
| [`docs/stage-committee-process.md`](docs/stage-committee-process.md) | Development process SOP (v5.0) |
| [`docs/lang-design/`](docs/lang-design/) | Language design documents (00-18) |
| [`docs/graph/`](docs/graph/) | Pipeline + data-flow diagrams |
| [`docs/develop/v0/stage-18/`](docs/develop/v0/stage-18/) | Stage 18 design docs + gate reviews |
| [`docs/develop/v0/tech-debt-register.md`](docs/develop/v0/tech-debt-register.md) | Tech debt tracking (all P0/P1/P2 resolved) |
| [`docs/develop/v0/v0.4-roadmap.md`](docs/develop/v0/v0.4-roadmap.md) | v0.4 roadmap design |
| [`docs/develop/v0/v0.5-roadmap.md`](docs/develop/v0/v0.5-roadmap.md) | v0.5 roadmap design |
| [`docs/tests/matrix.md`](docs/tests/matrix.md) | Global test matrix |
| [`docs/llvm/`](docs/llvm/) | LLVM integration docs |
| [`RELEASE_NOTES.md`](RELEASE_NOTES.md) | Version history (latest: v0.493.0, Stage 18.327) |
| [`docs/worklog.md`](docs/worklog.md) | Stage-by-stage work log |

### Recent Stage History

| Stage | Version | Summary |
|-------|---------|---------|
| 18.327 | v0.493.0 | P1 codegen bug 根因修复完成: opaque pointer migration — GEP typed ptr→opaque, load/store 加 ptr 前缀, entry block 加 br terminator (3 bugs, 总计 10 bugs fixed) |
| 18.326 | v0.493.0 | P1 codegen bug 根因修复: 7 bugs (bitcast→inttoptr/null, private→internal, zeroinitializer typed, ptr %self, globals ordering, emit_null_ptr value, LLVMSysEmitter lookup) |
| 18.325 | v0.493.0 | TD-CODEGEN-NEGATIVE final push: +60 codegen negative tests (8 categories) — 14.9%→23.3%, 25% target reached |
| 18.324 | v0.493.0 | TD-CODEGEN-NEGATIVE continued: +30 codegen negative tests (7 categories) — 10.7%→15.6% coverage |
| 18.323 | v0.493.0 | TD-CODEGEN-NEGATIVE: +24 codegen negative tests (6 categories) — 6.7%→10.7% coverage |
| 18.322 | v0.493.0 | TD-DUMMY-* 审计完成 (8 files, 250 Span::DUMMY 全部 Category A, 0 Category B 漏网) |
| 18.321 | v0.493.0 | Cargo.toml 过时注释清理 (description "LLVM 19" → "LLVM 22" + llvm-sys 注释 "19+21" → "18-22 default 22") |
| 18.320 | v0.493.0 | scripts/switch-llvm-version.sh 过时注释清理 (LLVM 19+21 → LLVM 18-22 default 22) |
| 18.319 | v0.493.0 | docs/ 子目录过时内容清理 (build-guide + testing-guide + graph/README + llvm/README) |
| 18.318 | v0.493.0 | 全量深度审查完成 (98 src files reviewed, 6 stale items fixed) — diagnostics/session/ast/resolve/lexer 审查通过 |
| 18.317 | v0.493.0 | mir/lower expr_variants doc-comment cleanup (4→3 arms) + deep module review |
| 18.316 | v0.493.0 | typeck/borrowck doc-comment cleanup (4 处过时引用) |
| 18.315 | v0.493.0 | 全项目门面文件审查 + lib.rs 精简 + stdlib placeholder 注释 + README 完全重构 |
| 18.312 | v0.493.0 | runtime.rs/prelude.rs 过时内容清理 |
| 18.310 | v0.493.0 | LOC 重构完全清零 (6 个文件 < 1500) |
| 18.304 | v0.493.0 | P3 field access on primitive 报错 |
| 18.297 | v0.493.0 | typeck gap: trait not implemented for type X |
| 18.295 | v0.493.0 | trait impl for primitive types (`impl MyTrait for i32`) |
| 18.293 | v0.493.0 | 禁止用户 inherent impl 原始类型 (类 Rust E0117) |
| 18.292 | v0.493.0 | inherent impl 冲突检测 (类 Rust "duplicate definitions") |
| 18.284 | v0.493.0 | TD-INTRINSIC-OVERUSE Phase 2-A (str::len/is_empty/as_bytes migrated to prelude) |
| 18.232 | v0.493.0 | 4 compound C helpers migrated to MIR intrinsics |
| 18.155 | v0.493.0 | landinc CLI build/run/new/check/clean |
| 18.151 | v0.493.0 | TD-CODEGEN-RESULT (CodegenResult propagation) |
| 18.103 | v0.371.0 | Per-Mono Codegen (TD-MONO-CODEGEN) |
| 18.96  | v0.364.0 | MIR optimization wiring (DCE + const_prop) |

---

## Development Process

Per `docs/stage-committee-process.md` (v5.0):

- **MUV (Minimum Verifiable Unit)**: each stage is atomic + independently testable
- **§3.2 acceptance gate**: `cargo build --release && fmt --check && clippy -- -D warnings && test --release` all green
- **§9.4.3 negative test ratio**: ≥ 25% negative tests (currently 27.8%)
- **§13.4 J1-J6 refactoring**: design unchanged / single responsibility / no circular deps / complete / same pipeline stage / LOC < 1500
- **§14.5 deep review**: mandatory at stage end
- **§19 packaging**: `landin-stage0-v<ver>-stage<N>-<desc>-r<rev>.tar.gz`

### LLVM Version

- **Default**: LLVM 22.1 (llvm-sys 221) — Stage 18.210 upgrade
- **Fallback**: LLVM 19.x (llvm-sys 191) — for environments without LLVM 22
- **Setup**: `source scripts/setup-llvm-env.sh` (auto-detects + installs)
- **Switch**: `bash scripts/switch-llvm-version.sh 22` (or 19)

---

## License

MIT License — see [`LICENSE`](LICENSE) for details.
