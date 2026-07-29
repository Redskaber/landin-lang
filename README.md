# Landin

**Author**: redskaber
**Version**: v0.84.0
**Date**: 2026-07-29

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance. The compiler is written in Rust and uses LLVM as its
backend via the `llvm-sys` crate.

> **⚠️ v0.1-rc3 — NOT YET READY FOR v0.1 RELEASE**
>
> Stage 14.1 capability assessment (`docs/develop/v0/stage-14/v0.1-capability-assessment.md`)
> identified **8 P0 blockers** that must be closed before v0.1. The prior
> "v0.1 GATE REACHED" claim (Stage 12.1, 2026-07-26) is **formally superseded**.
> This release (v0.84.0) is **v0.1-rc3** — architecture cleanup + API
> standardization + 6 gap closures (GAP-5/8/17/18/20/31) + method chain
> resolution + static method call correctness + impl method namespace fix
> + nested struct mutation + array of structs + LLVM module verification
> + Or-pattern fix + tuple/struct destructuring (let + match, nested, mixed)
> + function pointer support + LLVM 19 opaque pointer migration
> + mutual recursion + block-like statement boundary + zero-field struct
> methods + Bool store coercion + i64 constant width + field index ambiguity
> + integer cast generalization + float comparison writeback + bool match
> both arms + fn pointer forward reference + loop break value + enum &self
> match + deref on value + ref field access + tuple pattern match with
> literal sub-patterns + while+return parser fix + loop body divergence
> are complete, but deep soundness work (NLL fixpoint, region inference,
> drop elaboration, lifetime elision) remains.

---

## Status

| Component | Status |
|-----------|--------|
| Lexer / Parser / AST | ✅ Complete (344 tests) |
| HIR + Name Resolution | ✅ Complete (99 tests) |
| MIR + Typeck + Borrowck (NLL skeleton) | ✅ Complete (141 tests) |
| LLVM IR Codegen (TextEmitter) | ✅ Complete (309 tests) |
| LLVM Library Integration | ✅ `llvm-sys` v191/v211 linked (GAP-1 CLOSED) |
| LLVMSysEmitter (module builder) | ✅ 36/36 Emitter methods (1360 LOC) |
| Object file generation (`--emit-obj`) | ✅ LLVM Module → TargetMachine → .o |
| Linker + Executable (`--emit-bin`) | ✅ Auto C wrapper + cc link |
| `--run` flag | ✅ Compile → link → execute |
| Inline `println!` / `eprintln!` / `print!` output | ✅ Format args + no-newline + bool→"true"/"false" |
| if-let / while-let | ✅ TD-031 P0 CLOSED |
| Closures callable | ✅ TD-030 P0 CLOSED (inline approach) |
| 26 built-in macros | ✅ TD-032 P0 CLOSED |
| Trait resolver + vtable + dyn Trait | ✅ Stage 5 + Stage 7 complete |
| Codegen architecture (§14.4 split) | ✅ Stage 14.3 — `trait_dispatch/` split into vtable/dynptr/orchestrator |
| API naming standardization (§23) | ✅ Stage 14.4 — 0 glob re-exports |
| examples/ standardization (§17.4) | ✅ Stage 14.5 — 4 `[[example]]` targets |
| `self.x` field access in method bodies | ✅ Stage 13.18 — GAP-5 reclassified CLOSED |
| `print!` (no newline) | ✅ GAP-17 reclassified CLOSED |
| `run_ok` conformance runner | ✅ Stage 14.11 — GAP-8 CLOSED (actual `--run` execution + stdout/exit verification) |
| Bool → "true"/"false" printing | ✅ Stage 14.12 — GAP-18 CLOSED (via `emit_select`) |
| `&mut self` field mutation | ✅ Stage 14.19 — GAP-31 CLOSED (mutations propagate to caller) |
| `&self` method (read-only ref) | ✅ Stage 14.19 — works correctly with auto-deref |
| Array repeat `[val; N]` | ✅ Stage 14.20 — N-element array with proper `[T; N]` type |
| `&self`/`&mut self` + array field + index | ✅ Stage 14.21 — Deref+Index codegen + Ref auto-deref in field resolution |
| Nested struct construction | ✅ Stage 14.22 — proper field type resolution + struct type cache |
| Early return `return value;` | ✅ Stage 14.22-14.23 — block diverges → Never + is_terminated guard |
| `loop { break value; }` | ✅ Stage 14.24 — break value assigned to loop result local |
| Logical operators `&&`/`||` | ✅ Stage 14.24 — short-circuit evaluation verified |
| Bitwise operators `&`/`|`/`^`/`<<`/`>>` | ✅ Stage 14.24 — all verified |
| All compound assignment `+=`/`-=`/`*=`/`/=`/`%=` | ✅ Stage 14.25 — all verified |
| Enum unit variants + match | ✅ Stage 14.25 — verified |
| `i64` type | ✅ Stage 14.25 — verified |
| Comparison operators all branches | ✅ Stage 14.25 — `<=`/`>=`/`==`/`!=` verified |
| `*ptr = val` (deref store) | ✅ Stage 14.27 — pointer mutation through deref now works |
| `&i32` param + `&i32` return | ✅ Stage 14.27 — reference params and returns verified |
| Closure capture + inline call | ✅ Stage 14.28 — closures with captured variables work |
| Type cast `as` | ✅ Stage 14.28 — i32 as i64 verified |
| Match or-pattern `\|` | ✅ Stage 14.28 — `2 \| 3 => "small"` verified |
| Method return type propagation | ✅ Stage 14.29 — chained calls work with explicit annotations |
| Field error reporting (no silent defaults) | ✅ Stage 14.30-14.32 — type_errors sink + audit + revert (per §"报错 > 静默") |
| Control flow coverage (while+continue, nested loop, while+break) | ✅ Stage 14.33 — all verified |
| Match arm with `return` | ✅ Stage 14.34 — is_terminated guard prevents dead code overwriting return value |
| Struct-returning method calls without annotations | ✅ Stage 14.35-14.37 — fn_sigs threading + Call dest type writeback + Assign type propagation (fixpoint) |
| **Method chain resolution (multi-step + inline)** | ✅ Stage 14.38-14.40 — `a.add(b).scale(2).get()` works without annotations |
| **Static method calls (Type::method)** | ✅ Stage 14.41 — `Counter::new(5)` calls the method, not treated as struct ctor |
| **Method chain on all receiver types** | ✅ Stage 14.42 — Path/Call/Struct/MethodCall receivers all work + auto-deref for Ref |
| **Impl method namespace (no collision)** | ✅ Stage 14.42 — `A::new` + `B::new` no longer collide in value namespace |
| **Nested struct mutation (any depth)** | ✅ Stage 14.43 — `self.inner.val = v` works for 2-level + 3-level nesting |
| **Array of structs** | ✅ Stage 14.44 — `[Point{..}, Point{..}]` + `arr[i].field` + `arr[i].method()` |
| **LLVM module verification** | ✅ Stage 14.44 — catches invalid IR early (was silent empty output) |
| **Or-patterns in match** | ✅ Stage 14.45 — `1 \| 2 => { ... }` now correctly matches only listed values |
| **Tuple destructuring in let** | ✅ Stage 14.46 — `let (a, b, c) = (1, 2, 3)` now extracts fields correctly |
| **Tuple destructuring in match** | ✅ Stage 14.47 — `match t { (a, b) => ... }` now works (was garbage values) |
| **Struct destructuring (let + match)** | ✅ Stage 14.48 — `let Point { x, y } = p` + `match p { Point { x, y } => ... }` work |
| **Nested tuple destructure** | ✅ Stage 14.49 — `let ((a, b), c) = ((1, 2), 3)` works to any depth |
| **Nested/mixed pattern destructure** | ✅ Stage 14.50 — `let Outer { inner: Inner { a, b }, c } = o` + struct-tuple-field + tuple-of-structs |
| **Enum method resolution** | ✅ Stage 14.52 — `Color::Red.to_code()` now works (was silently returning 0) |
| **Function pointer parameters** | ✅ Stage 14.57-14.58 — `fn apply(f: fn(i32) -> i32, x: i32)` now works end-to-end |
| **LLVM 19 opaque pointers** | ✅ Stage 14.59 — all pointer types emit as `ptr` (LLVM 19 compliant) |
| **&i32 reference printing** | ✅ Stage 14.59 — `println!("{}", &val)` now prints the value (was `*`) |
| **&[i32; N] reference array indexing** | ✅ Stage 14.61 — `fn f(arr: &[i32; 3]) -> i32 { arr[0] }` now works |
| **&mut [i32; N] array element mutation** | ✅ Stage 14.62 — `fn modify(arr: &mut [i32; 3]) { arr[0] = 100; }` now works |
| **v0.1 Release Readiness** | **❌ NO-GO** — 7 P0 blockers remain (see below) |
| Conformance | 5153 tests (5026 compile + 127 run_ok with real runtime verification) |
| Rust tests | 1951 passed (with llvm-backend), 0 failed |
| Source code | ~90 files, ~32,000 LOC, 50+ modules |

---

## v0.1-rc3 Known Limitations (Remaining P0 Blockers)

The following P0 blockers remain after Stages 14.10-14.40 (which closed
GAP-5, GAP-8, GAP-17, GAP-18, GAP-20, GAP-31, and enabled method chain
resolution). These are **deferred to Stage 14.41+** (estimated 4-6 weeks
of focused work). See `docs/develop/v0/stage-14/v0.1-capability-assessment.md`
for the full gap analysis.

| ID | Gap | Impact | Status |
|----|-----|--------|--------|
| GAP-1 | NLL soundness regression — 229 conformance tests unsoundly flipped (Stage 13.25) | Unsound programs accepted (e.g., simultaneous `&mut x`) | Open |
| GAP-2 | Region inference is dead_code (no-op) | All regions erased; no lifetime constraint propagation | Open |
| GAP-3 | Drop elaboration is dead_code | No `Drop::drop` codegen; no `#[may_dangle]` dropck | Open |
| GAP-4 | Lifetime elision is dead_code | All lifetimes must be explicit; 3 elision rules not implemented | Open |
| GAP-6 | Two-phase borrows not implemented | `vec.push(vec.len())` pattern rejected | Open |
| GAP-9 | No real standard library | Only Rust-side `StdlibFacade` metadata; no Landin source for core/alloc/std | Open |
| GAP-21 | 229 conformance tests unsoundly flipped (couples with GAP-1) | Soundness regressions masked as test improvements | Open |
| GAP-30 | Trait method dispatch via `dyn` not verified end-to-end | `dyn Trait` method calls panic in codegen (panic fixed, runtime segfault deferred) | Partial |

**Recently closed** (Stages 14.10-14.40):
- ✅ GAP-5 (`self.x` field access) — reclassified CLOSED (fixed in Stage 13.18)
- ✅ GAP-8 (`run_ok` conformance runner) — CLOSED in Stage 14.11
- ✅ GAP-17 (`print!` no newline) — reclassified CLOSED (already works)
- ✅ GAP-18 (bool → "true"/"false") — CLOSED in Stage 14.12
- ✅ GAP-20 (void main return type UB) — reclassified CLOSED in Stage 14.16
- ✅ GAP-31 (`&mut self` field mutation) — CLOSED in Stage 14.19

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
cargo test                                       # all rust tests (1916, default)
cargo test --features llvm-backend               # all rust tests (1951, with LLVM)
python3 tests/conformance/run_all.py             # conformance suite (5082 tests)
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

### Hello World with Format Args

Both `fn main()` (Rust convention) and `fn landin_main()` (Landin convention)
are supported as entry points:

```bash
cat > /tmp/hello.lin << 'EOF'
fn main() -> i32 {
    let x = 42;
    let y = 99;
    println!("x = {}, y = {}", x, y);
    eprintln!("debug: x + y = {}", x + y);
    0
}
EOF

./target/release/landin-stage0 --run /tmp/hello.lin
# stdout: x = 42, y = 99
# stderr: debug: x + y = 141
# exit: 0
```

### Method Chain Example (Stage 14.40)

```bash
cat > /tmp/chain.lin << 'EOF'
struct V { x: i32, y: i32 }
impl V {
    fn new(x: i32, y: i32) -> V { V { x, y } }
    fn add(self, o: V) -> V { V { x: self.x + o.x, y: self.y + o.y } }
    fn scale(self, s: i32) -> V { V { x: self.x * s, y: self.y * s } }
    fn get(self) -> i32 { self.x + self.y }
}
fn main() -> i32 {
    let r = V::new(1, 2).add(V::new(3, 4)).scale(2).get();
    println!("{}", r);
    0
}
EOF

./target/release/landin-stage0 --run /tmp/chain.lin
# stdout: 20
# exit: 0
```

### Recursive Function Example

```bash
cat > /tmp/fib.lin << 'EOF'
fn fib(n: i32) -> i32 {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main() -> i32 {
    let r = fib(10);
    println!("fib(10) = {}", r);
    r
}
EOF

./target/release/landin-stage0 --run /tmp/fib.lin 2>/dev/null
# stdout: fib(10) = 55
# exit: 55
```

### Examples (API demos)

```bash
# Stage 14.5: examples/usage/ now runnable via cargo run --example
cargo run --example struct_call_codegen                        # compile + codegen
cargo run --example struct_compile_check                       # compile + error check
cargo run --example struct_variants_codegen                    # named/tuple structs
cargo run --example trait_dispatch_emission --features llvm-backend  # vtable + dynptr inspection
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

### Codegen Module Structure (post-Stage 14.3 §14.4 split)

```
src/codegen/
├── mod.rs                    (345 LOC) — public API + orchestration
├── emitter.rs                (663 LOC) — Emitter trait + EmitType + EmitValue
├── text/mod.rs               (650 LOC) — TextEmitter (text IR backend)
├── llvm/mod.rs              (1486 LOC) — LLVMSysEmitter (LLVM C API backend)
├── statement.rs              (279 LOC) — codegen_statement
├── rvalue.rs                 (323 LOC) — codegen_rvalue
├── operand.rs                (181 LOC) — codegen_operand + codegen_dyn_trait_call
├── terminator.rs             (298 LOC) — codegen_terminator
├── mir_translation.rs        (487 LOC) — type translation helpers
├── trait_dispatch/           (Stage 14.3 §14.4 split — was 962 LOC single file)
│   ├── mod.rs                 (57 LOC) — module declarations + re-exports
│   ├── vtable.rs             (337 LOC) — vtable global emission
│   ├── dynptr.rs             (268 LOC) — dynptr global emission
│   └── orchestrator.rs       (415 LOC) — combined emission + plan/summary
└── dyn_trait_emit.rs         (294 LOC) — dyn trait text emission
```

### Pipeline Stages

| Stage | Module | Status |
|-------|--------|--------|
| 0 | `lexer/`, `parser/`, `ast/` | ✅ Complete |
| 1 | `hir/`, `resolve/` | ✅ Complete |
| 2 | `mir/`, `typeck/`, `borrowck/` (NLL skeleton) | ✅ Complete (soundness gaps — GAP-1/2/3/4) |
| 3 | `codegen/` (TextEmitter) | ✅ Complete |
| 4 | Modules, closures, macros, benchmarks | ✅ Complete |
| 5 | `traits/`, `stdlib/` (TraitResolver, vtable, dyn Trait) | ✅ Complete |
| 6 | 47-module architecture refactoring | ✅ Complete |
| 7 | Region inference (TD-015), user-defined trait dyn (TD-018) | ✅ Data structures complete (activation deferred — GAP-2) |
| 8 | v0.2 features (lifetime elision, object safety, extern C, drop, async) | ⚠️ Data structures complete; activation deferred (GAP-3/4) |
| 9 | Parse conformance 600/600 | ✅ Complete |
| 10 | CLI upgrade + 8 conformance categories | ✅ Complete |
| 11 | Conformance 1139→5026, v0.1-rc1 | ✅ Complete |
| 12 | Cross-stage audits (r216/r217/r219) + v0.1-rc1 prep | ✅ Complete |
| 13 | LLVM execution pipeline + I/O + codegen refactoring | ✅ Complete (13.1-13.29) + backfilled (13.30-13.34) |
| **14** | **v0.1 release readiness — architecture cleanup + API standardization + gap closures + method chains** | **🔄 In Progress (14.1-14.40 ✅; 14.41+ P0 soundness work remains)** |
| 14.1 | v0.1 capability assessment + gap analysis | ✅ |
| 14.2 | Process hygiene: worklog backfill + version sync | ✅ |
| 14.3 | Architecture cleanup: `trait_dispatch.rs` split per §14.4 | ✅ |
| 14.4 | API naming audit (§23): 0 glob re-exports | ✅ |
| 14.5 | examples/ standardization (§17.4): 4 `[[example]]` targets | ✅ |
| 14.10-14.12 | GAP-5/8/17/18 closures (print!, bool printing, run_ok runner) | ✅ |
| 14.13-14.14 | GAP-30 dyn dispatch (panic fixed, runtime deferred) | ✅ |
| 14.16 | GAP-20 void main UB (reclassified CLOSED) | ✅ |
| 14.17-14.18 | run_ok expansion + GAP-31 investigation | ✅ |
| 14.19 | GAP-31 `&mut self` field mutation (Ref wrap + Deref+Field codegen) | ✅ |
| 14.20-14.22 | Array repeat + Deref+Index + Nested struct | ✅ |
| 14.23-14.25 | Return value + loop break + control flow coverage | ✅ |
| 14.26 | Pipeline test coverage matrix (620 paths, 99.7%) | ✅ |
| 14.27-14.28 | Deref store + closure capture + cast + match or-pattern | ✅ |
| 14.29-14.30 | Method return type + field error reporting | ✅ |
| 14.31-14.32 | Silent default audit + field error revert | ✅ |
| 14.33-14.34 | Control flow coverage + match+return | ✅ |
| 14.35-14.37 | Struct return without annotations (fn_sigs threading + writeback) | ✅ |
| 14.38-14.40 | Method chain resolution (resolver fix for impl/trait items) | ✅ |
| 14.41 | Static method call correctness (Type::method path resolution + adt_layouts re-populate) | ✅ |
| 14.42 | Method chain on all receiver types + impl method namespace fix (no collision) | ✅ |
| 14.43 | Nested struct mutation (2-level + 3-level) + recursive adt_layouts registration | ✅ |
| 14.44 | Array of structs + LLVM module verification (catches silent IR errors) + 5 bug fixes | ✅ |
| 14.45 | Or-pattern fix in match (was matching all values via wildcard) + audit (closures/strings/math) | ✅ |
| 14.46 | Tuple destructuring in let bindings (was outputting 0 0 0) + 2 run_ok tests | ✅ |
| 14.47 | Match arm tuple destructure (was garbage values) + skip SwitchInt on non-int scrutinee + 2 run_ok tests | ✅ |
| 14.48 | Struct destructuring in let + match (was 0 0 / garbage) + field-name→index lookup + 3 run_ok tests | ✅ |
| 14.49 | Nested tuple destructure (was 0 0 3) + recursive helper + 3 writeback steps + 2 run_ok tests | ✅ |
| 14.50 | Nested struct + mixed pattern destructure (was 0 0 3 / 0 0 99) + unified recursive helper + 3 run_ok tests | ✅ |
| 14.51+ | Deep P0 soundness work (NLL, region, drop, lifetime, stdlib, cargo) | ⏳ Deferred |

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
| `docs/llvm/stage-13.16-format-args.md` | 13.16 | Format args (`println!("{}", x)`) |

---

## Process & Governance

The project follows a structured stage-committee process for quality control.
All changes go through design alignment (§13.4), refactoring criteria (§14.4),
interface isolation (§16), API naming standard (§23), and design write-back
(§25.8). Core principles:

- **最优（长期 > 短期）** — prefer long-term soundness over short-term fixes
- **整体 > 局部** — global architecture > local optimization
- **显式 > 隐式** — explicit is better than implicit
- **报错 > 静默** — error out rather than silently accept bad input
- **去除兼容思维** — no compatibility hacks at v0.1 stage
- **通用 > 特例** — general mechanisms > special cases (avoid macros for what should be syntax)
- **少用特例** — minimize special cases (e.g., prefer syntax over macros)

- **Process SOP**: `docs/stage-committee-process.md` v3.21 (§0-§28)
- **API Naming Standard**: `docs/develop/v0/api-naming-standard.md`
- **v0.1 Capability Assessment**: `docs/develop/v0/stage-14/v0.1-capability-assessment.md`
- **Stage Dev Logs**: `docs/develop/v0/stage-{0..14}/`
- **Language Design**: `docs/lang-design/` (20 design documents)
- **Test Plans**: `docs/tests/v0/stage{0..14}/`
- **Pipeline Test Coverage**: `docs/tests/pipeline-test-coverage.md` (620 paths, 99.7% verified)
- **Worklog**: `docs/worklog.md` (mirror of `/home/z/my-project/worklog.md`)
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
| GAP-0 | P0 | ✅ CLOSED | Stage 14.2 (process hygiene + version sync) |
| GAP-5 | P0 | ✅ CLOSED | Stage 13.18 (self.x field access) |
| GAP-8 | P0 | ✅ CLOSED | Stage 14.11 (run_ok runner) |
| GAP-17 | P0 | ✅ CLOSED | Stage 14.11 (print! no newline) |
| GAP-18 | P0 | ✅ CLOSED | Stage 14.12 (bool printing) |
| GAP-20 | P0 | ✅ CLOSED | Stage 14.16 (void main UB) |
| GAP-31 | P0 | ✅ CLOSED | Stage 14.19 (&mut self mutation) |
| GAP-1, GAP-2, GAP-3, GAP-4, GAP-6, GAP-9, GAP-21, GAP-30 | P0 | Open | Stage 14.41+ (deferred — see assessment) |

---

## Verification

```bash
# Full CI/CD pipeline (as run by the maintainers)
cargo clean
cargo build --features llvm-backend
cargo fmt
cargo clippy --all-targets --features llvm-backend
cargo test --features llvm-backend
python3 tests/conformance/run_all.py
```

**Expected results** (v0.66.0):
- `cargo build --features llvm-backend`: succeeds
- `cargo fmt --check`: clean (no changes)
- `cargo clippy --all-targets --features llvm-backend`: 0 warnings, 0 errors
- `cargo test --features llvm-backend`: 1951 tests passed, 0 failed, 2 ignored
- `cargo build --examples --features llvm-backend`: 4 examples compile
- `conformance`: 5109 passed, 0 failed (5026 compile + 83 run_ok with runtime verification)

---

## License

MIT (see `LICENSE`).

---

## Repository

- **Repository**: https://github.com/redskaber/landin-lang
- **Authors**: redskaber
- **Categories**: compilers
- **Keywords**: compiler, language, systems-programming
