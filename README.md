# Landin

**Author**: redskaber

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance.

> **🔄 v0.1-rc1** — 前端完整 (parse + typeck + borrowck + IR emit, 5026/5000 conformance);
> 执行管线 pending (LLVM 集成 ✅, LLVMSysEmitter ✅, 链接 + 运行时 + `--run` — Stage 13.6-13.8)

## Status

| Component | Status |
|-----------|--------|
| Lexer / Parser / AST | ✅ Complete (344 tests) |
| HIR + Name Resolution | ✅ Complete (99 tests) |
| MIR + Typeck + Borrowck (NLL) | ✅ Complete (141 tests) |
| LLVM IR Codegen (text) | ✅ Complete (309 tests) |
| LLVM Library Integration | ✅ `llvm-sys` linked (GAP-1 CLOSED) |
| LLVMSysEmitter (module builder) | ✅ 36/36 Emitter methods (GAP-2 CLOSED) |
| Object file generation | 🔄 `to_object_file()` implemented, needs e2e test |
| Linker + Executable | ⏳ Stage 13.6 |
| Runtime + println! | ⏳ Stage 13.7 |
| `--run` flag | ⏳ Stage 13.8 |
| if-let / while-let | ✅ TD-031 P0 CLOSED (v0.22.0) |
| Closures callable | ✅ TD-030 P0 CLOSED (v0.23.0) |
| 26 built-in macros | ✅ TD-032 P0 CLOSED (v0.24.0) |
| Conformance | 5026/5000 (100.5%) — parse + typeck verified |
| Rust tests | 2286 passed, 0 failed |
| Benchmarks | 5 passed |

## Quick start

```bash
# ── LLVM environment setup ──
# Auto-detects LLVM version (19/21/etc) and updates .cargo/config.toml + Cargo.toml
source scripts/setup-llvm-env.sh
# Or manually:
bash scripts/switch-llvm-version.sh   # auto-detect
bash scripts/switch-llvm-version.sh 21  # force LLVM 21

# ── Build ──
cargo build --release                    # text IR only (default)
cargo build --release --features llvm-backend  # with LLVM library backend

# ── CLI usage ──
./target/release/landin-stage0 --emit-tokens path/to/file.ln
./target/release/landin-stage0 --emit-ast path/to/file.ln
./target/release/landin-stage0 --compile path/to/file.ln
./target/release/landin-stage0 --emit-llvm-ir path/to/file.ln

# ── Test ──
cargo test                               # all rust tests
python3 tests/conformance/run_all.py     # conformance suite (5026 tests)
```

## Architecture

```
source → lexer → parser → AST → HIR → resolve → MIR → typeck → borrowck → codegen → .ll
                                                                              ↓
                                                                    TextEmitter (String)
                                                                    LLVMSysEmitter (LLVM Module → .o)
```

| Stage | Module | Status |
|-------|--------|--------|
| 0 | `lexer/`, `parser/`, `ast/` | ✅ Complete |
| 1 | `hir/`, `resolve/` | ✅ Complete |
| 2 | `mir/`, `typeck/`, `borrowck/` | ✅ Complete |
| 3 | `codegen/` (TextEmitter) | ✅ Complete |
| 4 | modules, closures, macros, benchmarks | ✅ Complete |
| 5 | `traits/`, `stdlib/` (TraitResolver, vtable, dyn Trait) | ✅ Complete |
| 6 | 47-module architecture refactoring | ✅ Complete |
| 7 | Region inference (TD-015), user-defined trait dyn (TD-018) | ✅ Complete |
| 8 | v0.2 features (lifetime elision, object safety, extern C, drop, async) | ✅ Complete |
| 9 | Parse conformance 600/600 | ✅ Complete |
| 10 | CLI upgrade + 8 conformance categories | ✅ Complete |
| 11 | Conformance 1139→5026, v0.1-rc1 | ✅ Complete |
| 12 | Cross-stage audits (r216/r217/r219) + v0.1-rc1 prep (9/9 sub-stages incl. 12.8 final gate review) | ✅ Complete |
| 13 | v0.3 self-hosting prep | 🔄 In Progress |
| 13.1 | TD-028 §16 violation CLOSED | ✅ |
| 13.2 | TD-031 if-let/while-let P0 CLOSED | ✅ |
| 13.3a | TD-030 closures callable P0 CLOSED | ✅ |
| 13.4a | TD-032 26 built-in macros P0 CLOSED | ✅ |
| 13.5 | LLVM integration + LLVMSysEmitter | ✅ MUV-1+MUV-2 |
| 13.6-13.8 | Linker + Runtime + --run | ⏳ Pending |

## LLVM Integration

| Document | Description |
|----------|-------------|
| `docs/llvm/README.md` | LLVM integration overview |
| `docs/llvm/version-switching.md` | `switch-llvm-version.sh` usage |
| `docs/llvm/llvm-19-build-server-setup.md` | LLVM 19 setup (no root) |
| `docs/llvm/llvm-21-user-environment-setup.md` | LLVM 21 setup (system) |

## Documentation

- `docs/stage-committee-process.md` — Process SOP v3.21 (§1-§28)
- `docs/develop/v0/api-naming-standard.md` — API naming standard
- `docs/develop/v0/stage-{0..13}/` — Stage dev logs + gate reviews + plans
- `docs/lang-design/` — 19 language design documents
- `docs/tests/v0/stage{0..13}/` — Stage test plans
- `docs/worklog.md` — Worklog mirror
- `RELEASE_NOTES.md` — Release notes

## Process

- **v3.21** (§0-§28): §13.4 design alignment + §14.4 refactor governance + §25 deep review + §25.8 design write-back
- Stage 0-12 complete; Stage 13 in progress (LLVM integration + execution pipeline)
- Cross-stage audit (r216 first-pass, r217 second-pass, r219 Stage 12 deep review) complete
- Stage 12 ✅ COMPLETE (9/9 sub-stages, including 12.8 final gate review)
- §16 interface isolation compliant (TD-028 CLOSED)
- §17.1/§17.2/§18.4 docs compliant

## TD Status

| TD | Priority | Status |
|----|----------|--------|
| TD-028 | P2 | ✅ CLOSED (Stage 13.1) |
| TD-030 | P0 | ✅ CLOSED (Stage 13.3a) |
| TD-031 | P0 | ✅ CLOSED (Stage 13.2) |
| TD-032 | P0 | ✅ CLOSED (Stage 13.4a) |
| TD-029 | P2 | Open (deferred) |
| TD-033 | P1 | Open (Stage 13.5+) |

## License

MIT (see `LICENSE`).
