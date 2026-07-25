# Landin

**Author**: redskaber

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance.

> **Status:** v0.18.5 — Stage 0-10.5 in progress. Conformance 1991/5000 (39.8% of v0.1 gate).
> **2307 tests** + 5 benchmarks + 1991 conformance tests. 0 clippy warnings. fmt clean.
> Process v3.21 (§0-§28). §16 interface isolation compliant. §17.1/§17.2/§18.4 docs compliant.
>
> **Milestones:**
> - Stage 0-4: ✅ Complete (lexer, parser, HIR, MIR, typeck, borrowck, codegen)
> - Stage 5: ✅ Complete (99 sub-stages — TraitResolver, vtable, dyn Trait, stdlib)
> - Stage 6: ✅ Complete (18 sub-stages — 47-module architecture, all files < 1500 LOC)
> - Stage 7: ✅ Complete (9 sub-stages — TD-015 region inference, TD-018 user-defined trait dyn)
> - Stage 8: ✅ Complete (7 sub-stages — v0.2 roadmap + §25.8 + §25 deep review + §17 docs standardization)
> - Stage 9: ✅ Complete (12 sub-stages — parse conformance 600/600, reclassified as 'Parse conformance milestone')
> - Stage 10: ✅ Complete (8/9 sub-stages — Stage 10.8 §25 deep review PASS — v0.1 conformance 1139/5000, 22.8% of gate)
>
> **Architecture:** 50+ modules. All mod.rs/parser.rs/reader.rs/checker.rs/resolver.rs < 1500 LOC.
> Single responsibility per module. Data flows单向. Design docs synced (§25.8).
>
> **v0.1 roadmap:** conformance 1139/5000 (22.8%) — Stage 10 in progress (3 categories remaining)

## Quick start

```bash
# Build the compiler
cargo build --release

# Parse a Landin source file (tokens/AST)
./target/release/landin-stage0 --emit-tokens path/to/file.ln
./target/release/landin-stage0 --emit-ast path/to/file.ln

# Full compile (lex + parse + resolve + typeck + borrowck + codegen)
./target/release/landin-stage0 --compile path/to/file.ln

# Emit LLVM IR
./target/release/landin-stage0 --emit-llvm-ir path/to/file.ln

# Run conformance suite
python3 tests/conformance/run_all.py

# See examples/ for usage
```

## Architecture

```
source → lexer → parser → AST → HIR → resolve → MIR → typeck → borrowck → codegen → .ll
```

| Stage | Module | Status | Tests |
| ----- | -------- | -------- | ----- |
| 0 | `lexer/`, `parser/`, `ast/` | ✅ Complete | 344 |
| 1 | `hir/`, `resolve/` | ✅ Complete | 117 |
| 2 | `mir/`, `typeck/`, `borrowck/` | ✅ Complete | 170 |
| 3 | `codegen/` | ✅ Complete | 309 (incl. 5 §21 audit) |
| 4 | modules, closures, macros, benchmarks, ADR | ✅ Complete | 62 + 5 bench |
| 5 | `traits/`, vtable codegen, dyn Trait, stdlib, mini-cargo | ✅ Complete | 642 |
| 6 | architectural splits (47 modules) | ✅ Complete | — |
| 7 | region inference, user-defined trait dyn | ✅ Complete | 154 |
| 8 | v0.2 features + docs standardization | ✅ Complete | 38 |
| 9 | v0.1 parse conformance (600/600) | ✅ Complete (12/12) | +134 rust + 600 conformance |
| 10 | v0.1 full conformance (5000 target) | 🔄 In progress (5/9) | +26 rust + 359 conformance |

## API surface

Clean, §16-compliant public API. See `docs/develop/v0/api-naming-standard.md`.

| Entry point | Style |
|-------------|-------|
| `lexer::tokenize(src, &mut interner)` | free fn |
| `parser::parse_crate(tokens, &mut interner)` | free fn |
| `hir::lower::lower_crate(&ast, &interner)` | free fn |
| `resolve::resolve_crate(&mut hir, &interner)` | free fn |
| `mir::lower::lower_body(...)` / `lower_body_full(...)` | free fn |
| `TypeChecker::check_mir_body_with_tables(...)` | method (§16) |
| `BorrowChecker::check_mir_body(&mir)` | method |
| `codegen::codegen_crate(&CompileResult)` | free fn (§16) |
| `traits::TraitResolver` | struct (Stage 5.1) |
| `driver::compile(src)` | sole orchestrator |

## CLI

| Option | Description |
|--------|-------------|
| `--emit-tokens` | Output token stream only |
| `--emit-ast` | Output AST summary only |
| `--compile` | Full pipeline (lex + parse + resolve + typeck + borrowck + codegen) |
| `--emit-llvm-ir` | Output LLVM IR (implies --compile) |

## Error types

All error types implement `std::error::Error` + `Display`:

`LexError` · `ParseError` · `LowerError` · `ResolveError` · `TypeError` · `BorrowError`

## Codegen capabilities

| Feature | Example | LLVM IR |
| --------- | --------- | --------- |
| Function definition | `fn add(a: i32, b: i32) -> i32 { a + b }` | `define i32 @fn_0(...)` |
| Arithmetic | `1 + 2 * 3` | `mul nsw i32`, `add nsw i32` |
| Variables | `let x = 42;` | `alloca i32`, `store i32 42` |
| Control flow | `if a > b { 1 } else { 2 }` | `br i1 %cond, ...` |
| Loops | `while i < 10 { ... }` | `br label %loop` |
| Borrow/Deref | `&x` / `*r` | `store` / `load` |
| Structs | `struct Point { x: i32, y: i32 }` | `{ i32, i32 }` |
| Enums | `enum Color { Red, Green, Blue }` | `{ i32 }` (discriminant) |
| Closures | `\|x\| x + y` | `{ capture_fields }` struct |
| Macros | `println!("hello")` | unit (built-in expansion) |
| dyn Trait | `dyn Greet` | `{ ptr, ptr }` fat pointer + vtable |
| Overflow check | `a + b` | `call @__landin_panic_overflow` |

## Project layout

```
landin-stage0/
├── Cargo.toml              v0.18.5 (autotests=false — single all_tests target)
├── src/
│   ├── lexer/              Hand-written lexer (6 modules, reader.rs 349 LOC)
│   ├── parser/             Recursive-descent + Pratt parser (8 modules, parser.rs 263 LOC)
│   ├── ast/                AST node definitions
│   ├── hir/                HIR + lowering
│   ├── resolve/            Name resolution + scope + visibility (7 modules)
│   ├── mir/                MIR types + HIR→MIR lowering (7 modules + lower/)
│   ├── typeck/             Type inference + unification + lifetime elision (6 modules)
│   ├── borrowck/           NLL borrow checker + region inference (7 modules)
│   ├── codegen/            LLVM IR codegen via Emitter trait (5 modules)
│   ├── traits/             TraitResolver
│   ├── stdlib/             Standard library traits + vtable layout (3 modules)
│   ├── driver.rs           Full pipeline driver
│   └── bin/                CLI entry point (--emit-tokens/--emit-ast/--compile/--emit-llvm-ir)
├── tests/
│   ├── all_tests.rs        Unified entry point (#[path] mod declarations)
│   ├── common/mod.rs       Shared test helpers
│   ├── conformance/        .lin conformance suite + run_all.py (959 tests, target 5000)
│   │   ├── 00-parse/       (600 tests — Stage 9, 100% ✅)
│   │   ├── 01-typecheck/   (120 tests — Stage 10.1)
│   │   ├── 02-borrowck/    (80 tests — Stage 10.2)
│   │   ├── 03-codegen/     (61 tests — Stage 10.3)
│   │   ├── 04-e2e/         (48 tests — Stage 10.4)
│   │   ├── 05-soundness/   (50 tests — Stage 10.5)
│   │   └── run_all.py      Conformance runner (--mode auto/parse/compile)
│   └── v0/
│       ├── stage{0-9}/plan/  Stage 0-9 test files
│       └── stage10/plan/     Stage 10 test files (independent directory)
├── benches/                Performance benchmarks (5 benchmarks)
├── examples/               API demos + historical audit scripts
└── docs/
    ├── stage-committee-process.md  Process v3.21 (§13.4 + §14.4 + §25.8)
    ├── develop/v0/                 Dev logs + ADR + deep reviews + plans
    │   ├── stage-{0..9}/           Stage 0-9 dev logs + gate reviews + plans
    │   └── stage-10/               Stage 10 dev logs + gate reviews + plans (independent)
    ├── lang-design/                19 design docs (00-18) + CHANGELOG + FREEZE-REPORT
    ├── tests/                      Test plans + matrix
    │   └── v0/
    │       ├── stage{0..9}/        Stage 0-9 test plans
    │       └── stage10/            Stage 10 test plans (independent)
    └── worklog.md                  Worklog mirror (v3.18 §18.4.0)
```

## Testing

The test suite uses a **unified entry point** (`tests/all_tests.rs`) that
pulls in every test file under `tests/v0/stage{N}/plan/` via `#[path] mod`
declarations.

```bash
# Run all Rust tests
cargo test

# Run conformance suite (auto-detect parse vs compile mode)
python3 tests/conformance/run_all.py

# Run conformance in compile mode only
python3 tests/conformance/run_all.py --mode compile

# Run a single test module
cargo test --test all_tests -- lexer_tests

# Run benchmarks
cargo test --bench compile_bench -- --nocapture

# Format + lint
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Roadmap

- **Stage 0** ✅ Front-end (lexer + parser + AST)
- **Stage 1** ✅ HIR + name resolution
- **Stage 2** ✅ MIR + type check + borrow check (NLL)
- **Stage 3** ✅ LLVM codegen (§16 compliant)
- **Stage 4** ✅ COMPLETE (modules + closures + macros + benchmarks + ADR)
- **Stage 5** ✅ COMPLETE (99 sub-stages: TraitResolver + vtable + dyn Trait + stdlib)
- **Stage 6** ✅ COMPLETE (47-module architecture, all files < 1500 LOC)
- **Stage 7** ✅ COMPLETE (region inference + user-defined trait dyn)
- **Stage 8** ✅ COMPLETE (v0.2 roadmap + §25.8 + §25 deep review + §17 docs standardization)
- **Stage 9** ✅ COMPLETE (parse conformance 600/600 — reclassified as 'Parse conformance milestone')
- **Stage 10** 🔄 In progress (v0.1 full conformance 959/5000 — 19.2% of gate)
- **v0.1** = Stage 0 完整 + conformance 5000/5000 通过 (Stage 10.8 target)
- **v0.3** = self-hosting

## Documentation

- `docs/stage-committee-process.md` — Process SOP v3.21 (§1-§28, with §13.4 + §14.4 + §25.8)
- `docs/develop/v0/api-naming-standard.md` — API naming standard v2.30
- `docs/develop/v0/architecture-decisions.md` — 7 Architecture Decision Records
- `docs/develop/v0/stage-{0..9}/` — Stage 0-9 dev logs + gate reviews + plans (§17.3)
- `docs/develop/v0/stage-10/` — Stage 10 dev logs + gate reviews + plans (independent directory)
- `docs/lang-design/` — 19 language design documents (v1.3.2 Final, frozen)
- `docs/tests/v0/stage{0..9}/` — Stage 0-9 test plans (§17.2 双向印证)
- `docs/tests/v0/stage10/` — Stage 10 test plans (independent directory)
- `docs/tests/matrix.md` — Global test matrix
- `docs/worklog.md` — Worklog mirror (v3.18 §18.4.0) — synced through r210

## License

MIT (see `LICENSE`).
