# Landin

**Author**: redskaber  
**Version**: v0.160.0 (v0.2 Phase 2 started — NLL fixpoint design doc)  
**Date**: 2026-07-31

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance. The compiler is written in Rust and uses LLVM as its
backend via the `llvm-sys` crate.

> **v0.1 RELEASE CONFIRMED — Deep Audit + Data Structure Optimization Complete**
>
> Stages 14.80-14.112:
> - **Stage 14.101-14.104** — Deep audit: 22 P0 bugs identified, ALL 22 FIXED
> - **Stage 14.105** — Dead code cleanup: 1,013 LOC removed + perf baseline
> - **Stage 14.106-14.108** — Phase 2 architecture audit + 3 pre-v0.2 fixes
> - **Stage 14.109-14.112** — Data structure optimization: env var caching,
>   O(1) HirCrate lookup, UnificationTable HashMap→Vec, Terminator struct refactor
>
> **All 22 P0 bugs fixed. All 3 pre-v0.2 fixes done. 5 optimizations applied.**
> v0.1 is CONFIRMED READY. v0.2 can start safely.

> **v0.2 Phase 2 Started — Stage 15.34 (NLL fixpoint design doc)**
>
> Phase 1 complete (33 stages). Phase 2 started:
> - ✅ **Phase 1**: Ty interning, memory optimizations, writeback consolidation, diagnostics, HP-22
> - 🔧 **Phase 2**: NLL fixpoint design doc created (Stage 15.34)
>   - Next: implement fixpoint liveness analysis (Stage 15.35+)
>   - Unblocks: Drop elaboration, Region allocation, Closure redesign
>
> **Test count**: 173 lib tests + 2013 integration tests + 5216 conformance tests = 7402 passing.
> **0 clippy warnings**, fmt clean.

---

## Quick Start

### Build

```bash
# Build with LLVM backend (required for --emit-obj / --emit-bin / --run)
LLVM_SYS_191_PREFIX=/path/to/llvm-19 LLVM_LINK_SHARED=1 \
  cargo build --features llvm-backend

# Or use the included switcher script (auto-detects LLVM 19/21)
bash scripts/switch-llvm-version.sh
cargo build --features llvm-backend

# Frontend-only (no LLVM — supports --emit-tokens / --emit-ast / --compile)
cargo build
```

### Run a Landin program

```bash
# Compile + link + run
cargo run --features llvm-backend -- --run examples/hello.lin

# Emit LLVM IR
cargo run --features llvm-backend -- --emit-llvm-ir examples/hello.lin

# Emit object file
cargo run --features llvm-backend -- --emit-obj examples/hello.lin -o hello.o

# Compile only (no codegen output)
cargo run --features llvm-backend -- --compile examples/hello.lin
```

### Run tests

```bash
# Rust test suite (1951 tests)
cargo test --features llvm-backend

# Conformance suite (5171 .lin tests)
python3 tests/conformance/run_all.py

# Format + lint
cargo fmt
cargo clippy --all-targets --features llvm-backend
```

---

## Pipeline

```
Source Text (.lin)
    │
    ▼ [Stage 0] Lexer ──→ Vec<Token> + Vec<LexError>
    │
    ▼ [Stage 0] Parser ──→ Crate<ast::Item> + Vec<ParseError>
    │
    ▼ [Stage 1] HIR Lower ──→ HirCrate (owners, bodies, interner)
    │
    ▼ [Stage 1] Resolve ──→ mutates HIR (Res on paths)
    │
    ▼ [Stage 2] MIR Lower ──→ MirBody + UnificationTable
    │
    ▼ [Stage 2] TypeCheck ──→ mutates MIR (resolved types in local_decls)
    │
    ▼ [Stage 2] BorrowCheck (NLL) ──→ borrow errors
    │
    ▼ [Stage 3] Codegen ──→ LLVM IR (TextEmitter or LLVMSysEmitter)
    │
    ▼ [Stage 13] Link ──→ Object file → Executable
    │
    ▼ [Stage 13] Run ──→ Program output
```

**Two codegen backends**:
- `TextEmitter` — emits LLVM IR as text (debugging, `--emit-llvm-ir`)
- `LLVMSysEmitter` — uses `llvm-sys` C API for module building (`--emit-obj`,
  `--emit-bin`, `--run`); opaque pointer mode (LLVM 19+)

---

## Language Features

### Working in v0.114.0

- **Primitive types**: `i32`, `i64`, `f32`, `f64`, `bool`, `char`, `&str`
- **Compound types**: tuples, arrays `[T; N]`, structs, enums (with payload)
- **References**: `&T`, `&mut T` (NLL — last-use lifetimes, not lexical)
- **Borrow rules**: ✅ Sound (Stage 14.81 GAP-1 fix) — `let r1 = &mut x;
  let r2 = &mut x;` correctly rejected
- **Functions**: first-class, function pointers (`fn(i32) -> i32`)
- **Closures**: `|x| body` — captures by Copy or Move; struct captures work
  for ALL fields (Stage 14.82 + 14.84 audit fix); disjoint field captures
  (RFC 2229) deferred past v0.1
- **Methods**: `&self` / `&mut self` / `self` (by value); `Type::method()`
  static method calls; method chains; method calls on ref-bound locals
  (`let r = &p; r.method()`)
- **Trait dispatch**: vtable emission + `dyn Trait` fat pointer (infrastructure
  only — static trait method dispatch NOT supported in v0.1; trait impls can
  be defined but method calls crash at runtime)
- **Trait default bodies**: methods with default bodies that call other trait
  methods (`self.method()`) work via single-impl specialization (Stage 14.97)
- **Pattern matching**: literals, identifiers, tuples, structs, enums, or-patterns,
  nested patterns (any depth)
- **Destructuring**: `let (a, b) = ...;`, `let Point { x, y } = ...;`
- **Control flow**: `if`/`else`, `while`, `loop { break value; }`, `match`
  (with all pattern kinds including guards), early `return`, `break`, `continue`,
  `for i in start..end { body }` and `for i in start..=end { body }` (Stage 14.97)
- **Macros**: 26 built-in macros including `println!`, `eprintln!`, `print!`,
  `format!`, `assert!`, `assert_eq!`, `panic!`, `vec!`, `dbg!`, `todo!`
- **Operators**: all arithmetic, comparison, logical (`&&`/`||` short-circuit),
  bitwise (`&`/`|`/`^`/`<<`/`>>`), compound assignment (`+=` etc.)
- **Casts**: `expr as Type` (integer widening/narrowing, signed/unsigned)
- **Comments**: `// line`, `/* block */`

### Known limitations (v0.1)

- **GAP-2**: Region inference is dead_code (L3 infrastructure; `Erased`
  regions work as universal lifetime for v0.1 surface area)
- **GAP-3**: Drop elaboration is dead_code (L3; no user-defined `Drop::drop`)
- **GAP-4**: Lifetime elision is dead_code (L2; `Erased` works as universal)
- **GAP-7**: Disjoint closure captures (RFC 2229) — closures capture whole
  locals, not field-level disjoint captures
- **GAP-9**: No real standard library (only Rust-side `StdlibFacade` metadata)
- **GAP-14**: Cross-module visibility enforcement is stub
- **GAP-15**: Mini-cargo CLI not exposed (`landinc build/run/test` absent)
- **GAP-16**: No `landin test` / `landin fmt` / `landin doc` subcommands
- **For-loop over arrays**: only Range iterators (`start..end`, `start..=end`)
  are supported; arrays and other iterables produce a clear compile error
- **Trait default body with multiple impls**: uses first impl's self_ty for
  specialization (v0.1 single-impl heuristic; full monomorphization is v0.2+)
- **Trait default body calling another trait's method**: not supported
- **For-loop variable mutability**: loop variable is mutable even without
  `mut` annotation; modifying it affects iteration (P1, deferred to v0.2)

See `docs/develop/v0/stage-14/v0.1-capability-assessment.md` for the full
gap analysis.

---

## Architecture

```
src/
├── lexer/         Stage 0: source text → tokens
├── parser/        Stage 0: tokens → AST
├── hir/           Stage 1: AST → HIR
├── resolve/       Stage 1: name resolution (mutates HIR)
├── mir/           Stage 2: HIR → MIR
│   ├── lower/     MIR lowering (split per §14.4)
│   └── ty/        MIR type system
├── typeck/        Stage 2: type checking (unify + writeback)
├── borrowck/      Stage 2: borrow checking (NLL + liveness)
├── codegen/       Stage 3: MIR → LLVM IR
│   ├── llvm/      LLVMSysEmitter (llvm-sys C API)
│   ├── text/      TextEmitter (LLVM IR as text)
│   └── trait_dispatch/  vtable + dynptr + orchestrator (§14.4 split)
├── driver.rs      Orchestrator: runs all stages
├── session.rs     Span, source map, diagnostic emitter
├── stdlib/        StdlibFacade (Rust-side type metadata)
└── bin/           CLI driver (landin-stage0 binary)
```

**Process compliance**: `docs/stage-committee-process.md` v3.22
- §13.4 design alignment
- §14.4 architectural split (files < 1000 LOC)
- §23 API naming standard (no glob re-exports, `<verb>_<noun>` patterns)
- §25 deep review (8 dimensions, including D8 pipeline test coverage)

---

## Documentation

- `docs/stage-committee-process.md` — process spec (v3.22)
- `docs/develop/v0/stage-14/` — Stage 14 dev logs + gate reviews + plan
- `docs/develop/v0/stage-14/v0.1-capability-assessment.md` — gap analysis
- `docs/tests/pipeline-test-coverage.md` — pipeline path coverage matrix
- `docs/tests/matrix.md` — test matrix per stage
- `docs/llvm/` — LLVM-specific docs (build setup, stage-specific changes)
- `docs/tools/debug/` — debug tool docs
- `RELEASE_NOTES.md` — per-version release notes
- `docs/worklog.md` — full worklog (mirrored to `/home/z/my-project/worklog.md`)

---

## Test Counts (v0.118.0)

| Suite | Count | Pass rate |
|-------|-------|-----------|
| Rust unit/integration tests | 1951 | 100% |
| Conformance tests (.lin) | 5216 | 100% |
| - Parse-only (`00-parse`) | 600 | 100% |
| - Typecheck (`01-typecheck`) | 1020 | 100% |
| - Borrowck (`02-borrowck`) | 815 | 100% |
| - Codegen (`03-codegen`) | 601 | 100% |
| - End-to-end run (`04-e2e/06-run-ok`) | 170 | 100% |
| - Soundness (`05-soundness`) | 500 | 100% |
| - Stdlib (`06-stdlib`) | 502 | 100% |
| - Integration (`07-integration`) | 501 | 100% |
| Examples | 4 | 100% |
| Benchmarks | 5 | — |

---

## License

MIT (see `LICENSE`)

## Repository

https://github.com/redskaber/landin-lang

---

**Last updated**: 2026-07-30 (v0.126.0, Stage 14.112 — Terminator struct refactor)
