# Landin

**Author**: redskaber

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance.

> **Status:** v0.11.18 — Stage 0-4 complete, Stage 5 in progress.
> **1007 tests** + 5 benchmarks. 0 clippy warnings. fmt clean. 🎉 1000+ tests milestone!
> Process v3.20 (§0-§28). §16 interface isolation compliant.
> Stage 5.1-5.19: TraitResolver ✅ + driver integration ✅ + Copy detection ✅ + DefId→name ✅ + vtable ✅ + vtable codegen ✅ + dyn Trait fat-pointer ✅ + stdlib MVP ✅ + builtin Copy/Clone/Drop ✅ + primitive Copy auto-detect ✅ + Copy unification ✅ + trait impl statistics ✅ + trait method query API ✅ + trait hierarchy/supertraits ✅ + TraitResolver summary ✅ + vtable method resolution ✅ + trait coherence checking ✅ + trait impl completeness check ✅.

## Quick start

```bash
# Build the compiler
cargo build --release

# Compile a Landin source file (tokens/AST only — Stage 0 CLI)
./target/release/landin-stage0 --emit-tokens path/to/file.ln
./target/release/landin-stage0 --emit-ast path/to/file.ln

# Compile to LLVM IR (via the driver API)
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
| 5 | `traits/`, vtable codegen, dyn Trait, stdlib, mini-cargo | 🔄 In progress | 102 (TraitResolver + integration + Copy + DefId→name + vtable + vtable codegen + dyn fat-pointer + stdlib MVP + builtin Copy/Clone/Drop + primitive Copy auto-detect + Copy unification + trait impl statistics + trait method query API + trait hierarchy/supertraits + TraitResolver summary + vtable method resolution + trait coherence checking + trait impl completeness check) |

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
| `codegen::{Emitter, TextEmitter, EmitType, EmitValue}` | trait + impls |
| `traits::TraitResolver` | struct (Stage 5.1) |
| `driver::compile(src)` | sole orchestrator |

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
| Nested modules | `mod inner { pub fn f() {} }` | recursive module tree |
| Overflow check | `a + b` | `call @__landin_panic_overflow` |

## Project layout

```
landin-stage0/
├── Cargo.toml              v0.11.18 (autotests=false — single all_tests target)
├── src/
│   ├── lexer/              Hand-written lexer (109 tests)
│   ├── parser/             Recursive-descent + Pratt parser (85 tests)
│   ├── ast/                AST node definitions (150 tests)
│   ├── hir/                HIR + lowering (56 tests)
│   ├── resolve/            Name resolution + scope + visibility (43 tests)
│   ├── mir/                MIR types + HIR→MIR lowering (22 tests)
│   ├── typeck/             Type inference + unification (26 tests)
│   ├── borrowck/           NLL borrow checker (26 tests)
│   ├── codegen/            LLVM IR codegen via Emitter trait (294 tests)
│   ├── traits/             TraitResolver + vtable + coherence + completeness (Stage 5.1-5.19)
│   ├── driver.rs           Full pipeline driver
│   └── bin/                CLI entry point
├── tests/
│   ├── all_tests.rs        Unified entry point (37 #[path] mod declarations)
│   ├── common/mod.rs       Shared test helpers
│   ├── conformance/        .lin conformance suite + run_all.py
│   └── v0/stage{0-5}/plan/ Standardized test files (v3.17 §17.1)
├── benches/                Performance benchmarks (5 benchmarks)
├── examples/               API demos + historical audit scripts (v3.19 §17.4)
│   ├── usage/              Maintained API demos (MUST compile with current API)
│   ├── audit/              Archived stage gate review scripts (historical)
│   └── README.md           Index + run instructions
└── docs/
    ├── stage-committee-process.md  Process v3.20
    ├── develop/v0/                 Dev logs + ADR + deep reviews
    ├── tests/                      Test plans + matrix
    └── worklog.md                  Worklog mirror (v3.18 §18.4.0)
```

## Testing

The test suite uses a **unified entry point** (`tests/all_tests.rs`) that
pulls in every test file under `tests/v0/stage{N}/plan/` via `#[path] mod`
declarations. `Cargo.toml` sets `autotests = false` so only this single
target is built — keeping `Cargo.toml` compact (one `[[test]]` entry, not
19+) and producing a single test binary for faster incremental compilation.

```bash
# Run all tests (1017 expected — 1013 baseline + 3 vtable + 1 audit)
cargo test

# Run a single module (e.g. only lexer tests)
cargo test --test all_tests -- lexer_tests

# Run benchmarks
cargo test --bench compile_bench -- --nocapture

# Format + lint
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

To add a new test file: drop it under `tests/v0/stage{N}/plan/`, then add
one `#[path]` line to `tests/all_tests.rs` — no `Cargo.toml` edit needed.

## Roadmap

- **Stage 0** ✅ Front-end (lexer + parser + AST)
- **Stage 1** ✅ HIR + name resolution (use resolution, nested modules, visibility, unsafe impl/trait)
- **Stage 2** ✅ MIR + type check + borrow check (NLL, closures, coercion matrix)
- **Stage 3** ✅ LLVM codegen (§16 compliant, all soundness-critical limitations closed, L1 CLOSED)
- **Stage 4** ✅ COMPLETE (13 sub-stages: modules + PHI + visibility + closures + macros + benchmarks + ADR + v3.18)
- **Stage 5** 🔄 In progress (5.1: TraitResolver ✅; 5.2: Driver integration ✅; 5.3: ty_is_copy_with_resolver ✅; 5.4: DefId→name map ✅; 5.5: vtable ✅; 5.6: vtable codegen ✅; 5.7: dyn Trait fat-pointer ✅; 5.8: stdlib MVP ✅; 5.9: builtin Copy activation ✅; 5.10: builtin Clone/Drop + spec v3.20 ✅; 5.11: primitive Copy auto-detect ✅; 5.12: Copy unification ✅; 5.13: trait impl statistics ✅; 5.14: trait method query API ✅; 5.15: trait hierarchy/supertraits ✅; 5.16: TraitResolver summary ✅; 5.17: vtable method resolution ✅; 5.18: trait coherence checking ✅; 5.19: trait impl completeness check ✅ 🎉1000+ tests; next: dyn Trait MIR lowering, full stdlib, mini-cargo)
- **v0.1** = Stage 0 + conformance suite
- **v0.3** = self-hosting

## Documentation

- `docs/stage-committee-process.md` — Process SOP v3.18 (§1-§28)
- `docs/develop/v0/api-naming-standard.md` — API naming standard v1.5
- `docs/develop/v0/architecture-decisions.md` — 7 Architecture Decision Records
- `docs/develop/v0/stage-{0..5}/` — Per-stage dev logs + gate reviews + plans
- `docs/tests/` — Test plans + matrix + README
- `docs/worklog.md` — Worklog mirror (v3.18 §18.4.0)

## License

MIT (see `LICENSE`).
