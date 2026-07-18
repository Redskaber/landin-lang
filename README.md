# Landin

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance.

> **Status:** Stage 0 (front-end: lexer + parser + AST) is in deep review.
> Stage 1 (HIR + name resolution) is planned but not yet started.

## Quick start

```bash
# Build the Stage 0 compiler
cargo build --release

# Tokenize a Landin source file
./target/release/landin-stage0 --emit-tokens path/to/file.ln

# Print the AST
./target/release/landin-stage0 --emit-ast path/to/file.ln
```

## Project layout

```
landin-stage0/
├── Cargo.toml              Package manifest (v0.1.3)
├── src/
│   ├── lexer/              Hand-written character-level lexer (1415 lines)
│   ├── parser/             Recursive-descent + Pratt parser (1530 lines)
│   ├── ast/                AST node definitions (620 lines)
│   ├── session/            Span, FileId, SourceMap
│   ├── diagnostics/        Diagnostic buffer (placeholder)
│   └── bin/                CLI entry point
├── tests/
│   ├── lexer.rs            109 lexer tests (token-level + RP0 regression)
│   ├── parser.rs           85 parser tests (smoke + error-detection)
│   └── ast_structure.rs    125 AST structural + Pratt precedence + integration
└── docs/
    ├── build-guide.md      Build & test instructions
    ├── testing-guide.md    Test methodology
    ├── stage0-status.md    Status report (v0.1.3)
    └── development-log.md  S0-REV-7 development log
```

## Testing

```bash
# Run all 319 tests
cargo test

# Run with format + lint enforcement
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Language coverage (Stage 0)

- ✅ **Lexer**: literals (int/float/char/str/byte/raw), identifiers, raw
  identifiers (`r#match`), doc comments (`///` `//!`), 38 keywords, all
  operators with maximal munch
- ✅ **Parser**: all 11 item kinds (fn/const/static/struct/enum/trait/impl/
  type/extern/mod/use), 28 expression kinds, 12 pattern kinds, 16 type kinds,
  generic bounds (`T: Clone + Default`), where clauses, trait items, generic
  args in paths (`Vec<i32>`), `impl Trait` / `dyn Trait`, `pub(crate)` /
  `pub(in path)`, use groups/globs/aliases, struct literals, macro calls
  (`println!(...)`), `move` closures, `unsafe fn`, attribute parsing
  (`#[derive(Debug)]`)
- ✅ **AST**: complete node definitions with span tracking

See `docs/stage0-status.md` for the full feature matrix and known limitations.

## Roadmap

- **Stage 0** (Months 1-2): front-end closure — IN PROGRESS
- **Stage 1** (Months 3-4): HIR + name resolution — PLANNED
- **Stage 2** (Months 5-7): type check + borrow check (NLL on MIR)
- **Stage 3** (Months 8-9): LLVM codegen
- **Stage 4** (Month 10): macro system + attributes
- **Stage 5** (Months 11-12): mini-cargo + stdlib MVP
- **v0.1** = Stage 0 + conformance suite
- **v0.3** = self-hosting

## License

MIT (see `LICENSE`).

## Contributing

This is a single-developer project at present. The contribution workflow will
be formalized when Stage 1 begins. For now, please open an issue to discuss
any changes.
