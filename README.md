# Landin

**Author**: redskaber

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance.

> **Status:** Stage 0-3 complete + Stage 4 in progress. v0.9.8, 998 tests + 5 benchmarks,
> 46 review rounds. Process v3.17 (§15-§27).
> Stage 4.1-4.10: modules + PHI + visibility + closures + macro system.
> Stage 4.11: Benchmark suite ✅ + ADR docs ✅ (closes deep review R37 conditions).
> Next: L5 traits, L8 lli, user-defined macros.

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

| Stage | Module | Status |
| ------- | -------- | -------- |
| 0 | `lexer/`, `parser/`, `ast/` | ✅ Complete (344 tests — +1 unsafe impl/trait test in Stage 3.65) |
| 1 | `hir/`, `resolve/` | ✅ Complete (117 tests — nested modules + visibility enforcement) |
| 2 | `mir/`, `typeck/`, `borrowck/` | ✅ Complete (170 tests + 4 closure capture in Stage 4.7) |
| 3 | `codegen/` | ✅ Complete (294 tests + 5 §21 audit tests, LLVM IR text output; Stage 3.65: mir_type_to_emit_type docs) |

## API surface (Stage 3.63-3.66 naming standard)

The compiler exposes a clean, §16-compliant public API. See
`docs/develop/v0/api-naming-standard.md` for the full standard.

| Stage | Entry point | Style |
|-------|-------------|-------|
| 0 lexer | `lexer::tokenize(src, &mut interner)` | free fn |
| 0 parser | `parser::parse_crate(tokens, &mut interner)` | free fn (Stage 3.63 added) |
| 1.2 HIR lower | `hir::lower::lower_crate(&ast, &interner)` | free fn |
| 1.3 resolve | `resolve::resolve_crate(&mut hir, &mut interner)` | free fn (Stage 3.64: `use` decl resolution) |
| 2.1 MIR lower | `mir::lower::lower_body(...)` / `lower_body_full(...)` | free fn (Stage 3.65 aliases) |
| 2.2 typeck | `TypeChecker::check_mir_body_with_tables(...)` | method (§16-compliant) |
| 2.3 borrowck | `BorrowChecker::check_mir_body(&mir)` | method |
| 3 codegen | `codegen::codegen_crate(&CompileResult)` | free fn (§16-compliant) |
| 3 codegen (pluggable) | `codegen::{Emitter, TextEmitter, EmitType, EmitValue}` | trait + impls (Stage 3.64 re-export) |
| — driver | `driver::compile(src)` | sole orchestrator |

### Error types (Stage 3.64)

All error types implement `std::error::Error` + `Display`:

| Stage | Error type |
|-------|------------|
| 0 lexer | `LexError` |
| 0 parser | `ParseError` |
| 1.2 HIR lower | `LowerError` |
| 1.3 resolve | `ResolveError` |
| 2 typeck | `TypeError` |
| 2 borrowck | `BorrowError` |

This means they integrate with `?` propagation, `anyhow::Error`, `Box<dyn Error>`,
and the rest of the standard Rust error-handling ecosystem.

## Codegen capabilities (Stage 3)

| Feature | Example | LLVM IR |
| --------- | --------- | --------- |
| Function definition | `fn add(a: i32, b: i32) -> i32 { a + b }` | `define i32 @fn_0(i32 %arg0, i32 %arg1)` |
| Parameter passing | `add(3, 4)` | `call i32 @fn_0(i32 3, i32 4)` |
| Return | `42` | `ret i32 %v` |
| Arithmetic | `1 + 2 * 3` | `mul nsw i32`, `add nsw i32` |
| Comparison | `a > b` | `icmp sgt i32`, `zext i1` |
| Unary | `-x`, `!flag` | `sub i32 0`, `xor i32 -1` |
| Variables | `let x = 42;` | `alloca i32`, `store i32 42` |
| Control flow | `if a > b { 1 } else { 2 }` | `br i1 %cond, label %bb1, label %bb2` |
| Loops | `while i < 10 { ... }` | `br label %loop` |
| Borrow | `&x` | `store i32* %loc_x` |
| Deref | `*r` | `load i32, %ptr` |
| Recursive calls | `fib(n-1) + fib(n-2)` | `call i32 @fn_0(i32 %v)` |

## Project layout

```
landin-stage0/
├── Cargo.toml              Package manifest (v0.8.6)
├── src/
│   ├── lexer/              Lexer (109 tests)
│   ├── parser/             Recursive-descent + Pratt parser (85 tests)
│   ├── ast/                AST node definitions (149 tests)
│   ├── hir/                HIR + lowering + name resolution (451 tests)
│   ├── resolve/            Module + scope name resolution
│   ├── mir/                MIR types + HIR→MIR lowering
│   ├── typeck/             Type inference + unification
│   ├── borrowck/           NLL borrow checker
│   ├── codegen/            LLVM IR codegen (Emitter trait + TextEmitter)
│   ├── driver.rs           Full pipeline driver
│   └── bin/                CLI entry point
├── tests/
│   ├── codegen_tests.rs    26 codegen tests
│   ├── negative_cases.rs   33 negative-case tests (7/7 categories)
│   ├── deep_inspection.rs  15 structural verification tests
│   └── ...                 Lexer, parser, HIR, MIR, typeck tests
├── examples/
│   ├── stage2_4d_audit.rs  15-program audit
│   ├── round3-6_audit.rs   Multi-round negative-case audits
│   └── cross_stage_audit.rs 51-case cross-stage audit
└── docs/
    ├── stage-3-plan.md     Stage 3 plan
    ├── stage-committee-process.md  Process v3.5 (with §11 doc sync)
    └── ...                 Gate review reports, development logs
```

## Testing

```bash
# Run all 983 tests
cargo test

# Format + lint
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Roadmap

- **Stage 0** ✅ Front-end (lexer + parser + AST)
- **Stage 1** ✅ HIR + name resolution (Stage 3.64: `use` declaration resolution; Stage 3.65: `unsafe impl/trait` AST fields + `Res::SelfTy` discrimination)
- **Stage 2** ✅ MIR + type check + borrow check (6 rounds of review; Stage 3.65: `lower_body` aliases; Stage 3.66: `Lvalue`→`Place` rename)
- **Stage 3** ✅ LLVM codegen (COMPLETE — 37 review rounds, §16 compliant, all soundness-critical limitations closed; Stage 3.63-3.69 naming standardization + P2 fixes + deep review)
- **Stage 4** 🔄 In progress (4.1-4.10: modules+PHI+vis+closures+macros ✅; 4.11: benchmarks+ADR ✅; next: traits, lli)
- **Stage 5** Mini-cargo + stdlib MVP + trait dispatch (L5)
- **v0.1** = Stage 0 + conformance suite
- **v0.3** = self-hosting

## License

MIT (see `LICENSE`).
