# Landin

**Author**: redskaber  
**Version**: v0.25.1  
**Date**: 2026-07-27

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance. The compiler is written in Rust and uses LLVM as its
backend via the `llvm-sys` crate.

> **✅ v0.1-rc1** — Front-end complete (parse + typeck + borrowck + IR emit),
> 5026/5000 conformance tests passing, **execution pipeline operational with
> format args** (`--run` compiles, links, and executes Landin programs with
> `println!("x is {}", x)` correctly outputting `x is 42` — Stage 13.16).

---

## Status

| Component | Status |
|-----------|--------|
| Lexer / Parser / AST | ✅ Complete (344 tests) |
| HIR + Name Resolution | ✅ Complete (99 tests) |
| MIR + Typeck + Borrowck (NLL) | ✅ Complete (141 tests) |
| LLVM IR Codegen (TextEmitter) | ✅ Complete (309 tests) |
| LLVM Library Integration | ✅ `llvm-sys` v191/v211 linked (GAP-1 CLOSED) |
| LLVMSysEmitter (module builder) | ✅ 36/36 Emitter methods (GAP-2 CLOSED, 1360 LOC) |
| Object file generation (`--emit-obj`) | ✅ LLVM Module → TargetMachine → .o (Stage 13.6) |
| Linker + Executable (`--emit-bin`) | ✅ Auto C wrapper + cc link (Stage 13.7-13.10) |
| `--run` flag | ✅ Compile → link → execute (Stage 13.8) |
| Inline `println!` output (stdout) | ✅ Stage 13.13 (via `StatementKind::Println` → `printf`) |
| Inline `eprintln!` output (stderr) | ✅ Stage 13.14 (via `__landin_eprint`/`__landin_eprintf` helper) |
| Entry-point naming (`fn main()` AND `fn landin_main()`) | ✅ Stage 13.15 (strip-prefix fix) |
| **Format args (`println!("{}", x)`)** | **✅ Stage 13.16 (first real I/O feature)** |
| if-let / while-let | ✅ TD-031 P0 CLOSED (v0.22.0) |
| Closures callable | ✅ TD-030 P0 CLOSED (v0.23.0) |
| 26 built-in macros | ✅ TD-032 P0 CLOSED (v0.24.0) |
| String escape sequences (`\n`, `\t`, `\\`, `\"`) | ✅ Already supported (lexer `lex_escape()`) |
| Conformance | 5026/5000 (100.5%) — parse + typeck verified |
| Rust tests | 2338 passed, 0 failed |
| Benchmarks | 5 passed |
| Source code | ~90 files, ~32,000 LOC, 50+ modules |

---

## Quick Start

### Build

```bash
# ── LLVM environment setup ──
source scripts/setup-llvm-env.sh
# Or manually:
bash scripts/switch-llvm-version.sh       # auto-detect
bash scripts/switch-llvm-version.sh 21    # force LLVM 21

# ── Build ──
cargo build --release                            # text IR only (default)
cargo build --release --features llvm-backend    # with LLVM library backend

# ── Test ──
cargo test                                       # all rust tests (2333)
python3 tests/conformance/run_all.py             # conformance suite (5026 tests)
```

### CLI Usage

```bash
# ── Front-end output ──
./target/release/landin-stage0 --emit-tokens  path/to/file.ln
./target/release/landin-stage0 --emit-ast     path/to/file.ln
./target/release/landin-stage0 --compile      path/to/file.ln   # full compile check

# ── LLVM IR output ──
./target/release/landin-stage0 --emit-llvm-ir path/to/file.ln

# ── Object file generation (requires --features llvm-backend) ──
./target/release/landin-stage0 --emit-obj     path/to/file.ln -o prog.o

# ── Executable generation (auto C wrapper + cc link) ──
./target/release/landin-stage0 --emit-bin     path/to/file.ln -o prog

# ── Compile + link + execute in one step ──
./target/release/landin-stage0 --run          path/to/file.ln
echo $?    # → exit code (return value of landin_main())
```

### Hello World with Format Args (Stage 13.16)

Both `fn main()` (Rust convention) and `fn landin_main()` (Landin convention)
are supported as entry points (Stage 13.15 fix):

```bash
# Format args work — print computed values
cat > /tmp/hello.lin << 'EOF'
fn landin_main() -> i32 {
    let x = 42;
    let y = 99;
    println!("x = {}, y = {}", x, y);     # → stdout: x = 42, y = 99
    eprintln!("debug: x + y = {}", x + y); # → stderr: debug: x + y = 141
    0
}
EOF

./target/release/landin-stage0 --run /tmp/hello.lin
# stdout: x = 42, y = 99
# stderr: debug: x + y = 141
# exit: 0

# Separate streams with redirection
./target/release/landin-stage0 --run /tmp/hello.lin > /tmp/out.txt 2> /tmp/err.txt
cat /tmp/out.txt  # → x = 42, y = 99
cat /tmp/err.txt  # → debug: x + y = 141
```

### Supported Format Placeholders (v0.1)

| Placeholder | Types | Output |
|-------------|-------|--------|
| `{}` | i32/i64/u32/u64/bool | Decimal integer (via `%ld`) |
| `{}` | f32/f64 | Float (via `%f`) |
| `{}` | &str / &[u8] | String (via `%s`) |

`{:?}` (debug), `{:x}`/`{:o}`/`{:b}` (hex/octal/binary), and padding/alignment
are deferred to v0.2 (require full macro_rules! expansion).

### Recursive Function Example

```bash
cat > /tmp/fib.lin << 'EOF'
fn fib(n: i32) -> i32 {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}
fn landin_main() -> i32 {
    let r = fib(10);
    println!("fib(10) = {}", r);
    r
}
EOF

./target/release/landin-stage0 --run /tmp/fib.lin 2>/dev/null
# stdout: fib(10) = 55
# exit: 55
```

---

## Architecture

```
source → lexer → parser → AST → HIR → resolve → MIR → typeck → borrowck → codegen
                                                                              ↓
                                                                    ┌─────────────────┐
                                                                    │  TextEmitter    │ → .ll (text IR for inspection)
                                                                    │  LLVMSysEmitter │ → .o (object file)
                                                                    └─────────────────┘
                                                                              ↓
                                                                    cc wrapper.c prog.o -o exe -lm
                                                                    (wrapper provides: main(), __landin_panic_*,
                                                                     __landin_eprint, __landin_eprintf for stderr)
                                                                              ↓
                                                                    ./exe (calls landin_main())
```

### Pipeline Stages

| Stage | Module | Status |
|-------|--------|--------|
| 0 | `lexer/`, `parser/`, `ast/` | ✅ Complete |
| 1 | `hir/`, `resolve/` | ✅ Complete |
| 2 | `mir/`, `typeck/`, `borrowck/` | ✅ Complete |
| 3 | `codegen/` (TextEmitter) | ✅ Complete |
| 4 | Modules, closures, macros, benchmarks | ✅ Complete |
| 5 | `traits/`, `stdlib/` (TraitResolver, vtable, dyn Trait) | ✅ Complete |
| 6 | 47-module architecture refactoring | ✅ Complete |
| 7 | Region inference (TD-015), user-defined trait dyn (TD-018) | ✅ Complete |
| 8 | v0.2 features (lifetime elision, object safety, extern C, drop, async) | ✅ Complete |
| 9 | Parse conformance 600/600 | ✅ Complete |
| 10 | CLI upgrade + 8 conformance categories | ✅ Complete |
| 11 | Conformance 1139→5026, v0.1-rc1 | ✅ Complete |
| Stage 12 | Cross-stage audits (r216/r217/r219) + v0.1-rc1 prep | ✅ Complete |
| 13 | v0.3 self-hosting prep + LLVM execution pipeline | 🔄 In Progress |
| 13.1 | TD-028 §16 violation CLOSED | ✅ |
| 13.2 | TD-031 if-let/while-let P0 CLOSED | ✅ |
| 13.3a | TD-030 closures callable P0 CLOSED | ✅ |
| 13.4a | TD-032 26 built-in macros P0 CLOSED | ✅ |
| 13.5-13.10 | LLVM integration + `--emit-obj` + `--emit-bin` + `--run` | ✅ |
| 13.11-13.12 | `println!` capture + side-table emission | ✅ |
| 13.13 | Inline `println!` emission via `StatementKind::Println` | ✅ |
| 13.14 | `eprintln!`/`eprint!` stderr emission | ✅ |
| 13.15 | Fix `landin_main` double-prefix symbol bug | ✅ |
| **13.16** | **Format args (`println!("{}", x)`) — first real I/O feature** | ✅ |
| 13.17+ | print-flush, bool→"true"/"false", v0.2 macro_rules! | 🔄 Pending |

---

## LLVM Integration

The compiler supports LLVM 19 (build server) and LLVM 21 (user environment)
via the `llvm-sys` crate. The `LLVMSysEmitter` (1360 LOC) implements all 36
`Emitter` trait methods, building real LLVM modules via the C API.

### Environment Setup

| Script | Purpose |
|--------|---------|
| `scripts/setup-llvm-env.sh` | Auto-detect + download LLVM 19 dev packages (no root) |
| `scripts/switch-llvm-version.sh` | Switch between LLVM 19/21 configurations |

### LLVM Documentation

| Document | Stage | Description |
|----------|-------|-------------|
| `docs/llvm/README.md` | 13.5+ | LLVM integration overview + environment setup |
| `docs/llvm/version-switching.md` | 13.5 | Switching between LLVM 19 and 21 |
| `docs/llvm/llvm-19-build-server-setup.md` | 13.5 | LLVM 19 setup (build server, no root) |
| `docs/llvm/llvm-21-user-environment-setup.md` | 13.5 | LLVM 21 setup (user environment) |
| `docs/llvm/stage-13.6-object-file-generation.md` | 13.6 | `--emit-obj` flag implementation |
| `docs/llvm/execution-pipeline.md` | 13.8-13.10 | End-to-end execution pipeline |
| `docs/llvm/stage-13.13-println-inline-emission.md` | 13.13 | Inline `println!` emission |
| `docs/llvm/stage-13.14-eprintln-stderr-emission.md` | 13.14 | `eprintln!`/`eprint!` stderr emission |
| `docs/llvm/stage-13.16-format-args.md` | **13.16** | **Format args (`println!("{}", x)`)** |

---

## Process & Governance

The project follows a structured stage-committee process for quality control.
All changes go through design alignment (§13.4), refactoring criteria (§14.4),
interface isolation (§16), and design write-back (§25.8).

- **Process SOP**: `docs/stage-committee-process.md` v3.21 (§0-§28)
- **API Naming Standard**: `docs/develop/v0/api-naming-standard.md`
- **Stage Dev Logs**: `docs/develop/v0/stage-{0..13}/`
- **Language Design**: `docs/lang-design/` (19 design documents)
- **Test Plans**: `docs/tests/v0/stage{0..13}/`
- **Worklog**: `docs/worklog.md`
- **Release Notes**: `RELEASE_NOTES.md`

---

## Technical Debt Status

| TD | Priority | Status | Stage Closed |
|----|----------|--------|--------------|
| TD-028 | P2 | ✅ CLOSED | Stage 13.1 (§16 violation fix) |
| TD-030 | P0 | ✅ CLOSED | Stage 13.3a (closures callable) |
| TD-031 | P0 | ✅ CLOSED | Stage 13.2 (if-let/while-let) |
| TD-032 | P0 | ✅ CLOSED | Stage 13.4a (26 built-in macros) |
| TD-029 | P2 | Open | Deferred (TyKind::Dynamic refactor) |
| TD-033 | P1 | Open | Stage 13.5+ (6 P1 sub-items) |

**🎉 All 3 P0 items CLOSED** — v0.3 self-hosting preparation complete.

---

## Verification

```bash
# Full CI/CD pipeline (as run by the maintainers)
cargo clean
cargo build --lib --features llvm-backend
cargo fmt
cargo clippy --all-targets
cargo test
python3 tests/conformance/run_all.py
```

**Expected results** (v0.25.1):
- `cargo build`: succeeds
- `cargo fmt`: clean (no changes)
- `cargo clippy`: 0 warnings, 0 errors
- `cargo test`: 2333 tests passed, 0 failed
- `conformance`: 5026 passed, 0 failed

---

## License

MIT (see `LICENSE`).

---

## Repository

- **Repository**: https://github.com/landin-lang/main
- **Authors**: redskaber
- **Categories**: compilers
- **Keywords**: compiler, language, systems-programming
