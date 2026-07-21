# Landin

**Author**: redskaber

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance.

> **Status:** Stage 0-2 complete (lexer, parser, HIR, name resolution, MIR,
> type checking, borrow checking). Stage 3 (LLVM codegen) in progress —
> v0.8.6, 929 tests passing, 20 gate review rounds passed (audit CONVERGED).
> Process v3.13 (§15 最优 > 最小 + §16 阶段间接口隔离 + §17 测试矩阵全覆盖
> + §18 轮次完成文档同步).

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
| 0 | `lexer/`, `parser/`, `ast/` | ✅ Complete (245 tests) |
| 1 | `hir/`, `resolve/` | ✅ Complete (451 tests) |
| 2 | `mir/`, `typeck/`, `borrowck/` | ✅ Complete (673 tests, 6 rounds of review) |
| 3 | `codegen/` | 🔄 In progress (182 tests, LLVM IR text output) |

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
# Run all 699 tests
cargo test

# Format + lint
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Roadmap

- **Stage 0** ✅ Front-end (lexer + parser + AST)
- **Stage 1** ✅ HIR + name resolution
- **Stage 2** ✅ MIR + type check + borrow check (6 rounds of review)
- **Stage 3** 🔄 LLVM codegen (MVP complete, 20 gate review rounds CONVERGED, L-PIPE-1 + L-ENUM-UNION + L-ENUM-BINDING + L13 fat ptr + slice/str indexing + element type propagation closed)
- **Stage 4** Macro system + attributes
- **Stage 5** Mini-cargo + stdlib MVP
- **v0.1** = Stage 0 + conformance suite
- **v0.3** = self-hosting

## License

MIT (see `LICENSE`).
