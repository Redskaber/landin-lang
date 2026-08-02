# Landin

**Author**: redskaber
**Version**: v0.196.0 (v0.2 Phase 2 — fn_sigs region inference)
**Date**: 2026-08-02

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance. The compiler is written in Rust and uses LLVM as its
backend via the `llvm-sys` crate.

> **v0.2 Phase 3 — Task 13 (impl Drop + RAII) ✅ FULLY COMPLETE**
>
> Stage 15.61 resolved all four root causes preventing `impl Drop` programs from
> compiling end-to-end. Stage 15.62 completed the Drop semantics with correct
> reverse declaration drop order and double-drop prevention.
>
> **Stage 15.61 — Four bugs fixed**:
> 1. `elaborate_drops` infinite loop (OOM kill, exit 137) — `StorageDead` no
>    longer carried into the new block when splitting.
> 2. Drop codegen type mismatch — pass `OpaquePtr` (not value type) to `emit_call`.
> 3. LLVM backend missing drop glue emission — added `emit_drop_glue_functions`
>    call to `codegen_crate_to_module`.
> 4. borrowck treated Drop as a read — now treats as destructor (no-op for
>    moved, consuming for live).
>
> **Stage 15.62 — Drop order + double-drop prevention**:
> 1. Drop order: `StorageDead` emission reversed → reverse declaration order
>    (matches Rust RFC 1327).
> 2. Double-drop prevention: `collect_moved_locals` flow-insensitive analysis
>    skips moved temporaries in `elaborate_drops`.
>
> **Stage 15.63 — Recursive drop (fields with Drop)**:
> `emit_drop_glue_functions` rewritten to emit drop glue for ALL types
> needing drop (not just types with `impl Drop`). Structs without `impl Drop`
> but with Drop fields now get drop glue that recursively drops each field
> via GEP + call.
>
> **Stage 15.64 — Struct literal Copy→Move + field-copy prevention**:
> Struct literals now use `Operand::Move` for non-Copy field types (was
> always `Copy`). Field access temps (`o.inner`) are excluded from drop
> (they're views, not owned values). Added shared `is_mir_ty_copy_conservative`
> helper in `mir::ty` (DRY). Runtime verified: 4 drops → 2 drops (correct).
>
> **Stage 15.65 — HP-22 cleanup (Task 16 COMPLETE)**:
> Removed the legacy `dyn_trait_calls` side-table from `MirBody`. The dyn
> Trait call info is now solely on `TerminatorKind::Call`'s `dyn_trait_call`
> field (Stage 15.30). Removed legacy `codegen_dyn_trait_call` function +
> legacy codegen dispatch path. 6 test files updated.
>
> **Stage 15.66 — Recursive drop for enums (Task 13 drop semantics complete)**:
> `emit_drop_glue_functions` now handles `AdtLayout::Enum` — loads discriminant,
> emits `SwitchInt` to dispatch to active variant's block, GEPs to payload
> fields, calls `drop_adt_<fieldDefId>`. Runtime verified: enum with impl Drop
> + Drop variant produces "enum dropped" then "inner dropped" (correct order).
> Task 13 drop semantics now fully complete for structs AND enums.
>
> **Stage 15.67 — True Rust NLL (Task 7 truly complete, GAP-1 rejected)**:
> Per §1.0 原則 9 "正确 > 妥协": removed the `ever_read` guard (Option B
> compromise) from `kill_expired_borrows_dataflow`. Now uses true
> liveness-based NLL. Fixed `&mut self` false positive via kill-after-call
> semantics. Added block-entry kill + StorageLive/StorageDead handling.
> Flipped 108 conformance tests from compile_error to compile_ok (valid NLL
> programs now accepted). Task 7 (HP-10) TRULY COMPLETE — real NLL, not
> lexical lifetimes.
>
> **Stage 15.68 — Remove dead NLL code**: Removed `compute_last_use_map`,
> `compute_ever_read`, `LastUseMap` — dead code from the GAP-1 compromise.
>
> **Stage 15.69 — v0.2 milestone gate review**: Comprehensive review of all
> 68 stages. 8/20 tasks COMPLETE, 5/8 success criteria met. v0.2
> SUBSTANTIALLY COMPLETE — remaining: Task 12 (Lifetime elision) or
> Task 20 (Box<T> in prelude).
>
> **Runtime verified**: `let a,b,c` with Drop produces "dropping 3, 2, 1"
> (reverse order, no duplicates) — matches Rust exactly.
>
> **Test count**: 226 lib tests + 2133 integration tests + 5216 conformance
> tests = **7567 passing**. 0 clippy warnings, fmt clean.

> **v0.2 Phase 2 — Soundness Closures SUBSTANTIALLY COMPLETE**
>
> - ✅ **Phase 1**: Ty interning, memory optimizations, writeback consolidation,
>   diagnostics, HP-22
> - ✅ **Phase 2 Task 7 (HP-10)**: Fixpoint dataflow NLL — migration COMPLETE
> - ✅ **Phase 2 Task 8 (HP-12)**: Drop elaboration — pipeline COMPLETE
>   (Stages 15.42-15.47, 15.55-15.61)
> - 🔧 **Phase 2 Task 9 (HP-5)**: Region allocation — infrastructure
>   integrated, simplified constraints (Stages 15.48-15.52)
> - 🔧 **Phase 2 Task 10 (HP-3)**: Closure redesign — design only (Stage 15.53)
> - ✅ **Phase 3 Task 13**: `impl Drop` + RAII — **COMPLETE** (Stage 15.61)

---

## Quick Start

### Build

```bash
# Build with LLVM backend (required for --emit-obj / --emit-bin / --run)
LLVM_SYS_191_PREFIX=/path/to/llvm-19 LLVM_LINK_SHARED=1 \
  cargo build --release --features llvm-backend

# Or use the included switcher script (auto-detects LLVM 19/21)
bash scripts/switch-llvm-version.sh
cargo build --release --features llvm-backend

# Frontend-only (no LLVM — supports --emit-tokens / --emit-ast / --compile)
cargo build
```

### Run a Landin program

```bash
# Compile + link + run
cargo run --release --features llvm-backend -- --run examples/hello.lin

# Emit LLVM IR
cargo run --release --features llvm-backend -- --emit-llvm-ir examples/hello.lin

# Emit object file
cargo run --release --features llvm-backend -- --emit-obj examples/hello.lin -o hello.o

# Compile only (no codegen output)
cargo run --release --features llvm-backend -- --compile examples/hello.lin
```

### Run an `impl Drop` program (new in v0.187.0)

```landin
// counter_drop.lin — RAII counter with Drop impl
struct Counter { value: i32 }

impl Drop for Counter {
    fn drop(self: &mut Counter) {
        let _ = self.value;  // destructor reads the value
    }
}

fn make(v: i32) -> Counter {
    Counter { value: v }
}

fn use_counter(c: &Counter) -> i32 {
    c.value
}

fn main() -> i32 {
    let c = make(10);
    let d = make(20);
    let sum = use_counter(&c) + use_counter(&d);
    sum  // → exit 30
}
```

```bash
LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
  cargo run --release --features llvm-backend -- --run counter_drop.lin
# → exit 30
```

### Run tests

```bash
# Rust test suite (2351 tests: 221 lib + 2130 integration)
LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
  cargo test --features llvm-backend

# Conformance suite (5216 .lin tests)
python3 tests/conformance/run_all.py

# Format + lint
cargo fmt
LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 \
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
    ▼ [Stage 2] Drop Elaboration ──→ inserts Drop terminators (v0.2 Stage 15.44)
    │
    ▼ [Stage 2] BorrowCheck (NLL dataflow) ──→ borrow errors
    │
    ▼ [Stage 3] Codegen ──→ LLVM IR (TextEmitter or LLVMSysEmitter)
    │   ├── Drop glue emission (v0.2 Stage 15.57 + 15.61)
    │   ├── Vtable + dynptr globals
    │   └── Function bodies (MIR → LLVM IR)
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

### Working in v0.187.0

- **Primitive types**: `i32`, `i64`, `f32`, `f64`, `bool`, `char`, `&str`
- **Compound types**: tuples, arrays `[T; N]`, structs, enums (with payload)
- **References**: `&T`, `&mut T` (NLL dataflow — last-use lifetimes, Stage 15.41)
- **Borrow rules**: ✅ Sound (Stage 14.81 GAP-1 fix + Stage 15.61 Drop semantics)
- **Functions**: first-class, function pointers (`fn(i32) -> i32`)
- **Closures**: `|x| body` — captures by Copy or Move (TD-030 inline lowering)
- **Methods**: `&self` / `&mut self` / `self` (by value); `Type::method()`
  static method calls; method chains; method calls on ref-bound locals
- **Trait dispatch**: vtable emission + `dyn Trait` fat pointer (Stage 7.6
  user-defined trait dyn support, TD-018)
- **Trait default bodies**: methods with default bodies that call other trait
  methods work via single-impl specialization (Stage 14.97)
- **`impl Drop` + RAII** ✅ NEW (Stage 15.61-15.63): `impl Drop for T` blocks
  generate `drop_adt_<DefId>` glue functions that call the user's
  `Drop::drop` method; `elaborate_drops` inserts `Drop` terminators in
  **reverse declaration order** (matching Rust RFC 1327); borrow checker
  treats `Drop` as a destructor (no-op for moved, consuming for live);
  `collect_moved_locals` prevents double-drop of moved temporaries;
  **recursive drop** — structs without `impl Drop` but with Drop fields
  get drop glue that recursively drops each field via GEP + call.
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

### Known limitations (v0.2)

- **Drop order**: ✅ FIXED (Stage 15.62) — locals are now dropped in reverse
  declaration order, matching Rust RFC 1327
- **Double-drop**: ✅ FIXED (Stage 15.62) — `collect_moved_locals` prevents
  double-drop of moved temporaries
- **Partial moves**: not supported (whole-value moves only)
- **Drop flags**: not implemented (conditional control flow with Drop types
  may produce leaks for conditionally-moved locals — full runtime drop flags
  deferred to v0.3)
- **Recursive drop**: dropping fields that themselves need drop (when the
  parent doesn't have `impl Drop`) — deferred to v0.3
- **`Box<T>` in prelude**: blocked on recursive drop (deferred to v0.3)
- **GAP-2**: Region inference is simplified (Erased regions work as universal
  lifetime for the v0.2 surface area)
- **GAP-4**: Lifetime elision is partial (Erased works as universal)
- **GAP-7**: Disjoint closure captures (RFC 2229) — closures capture whole
  locals, not field-level disjoint captures
- **GAP-9**: No real standard library (only Rust-side `StdlibFacade` metadata)
- **GAP-14**: Cross-module visibility enforcement is stub
- **GAP-15**: Mini-cargo CLI not exposed (`landinc build/run/test` absent)
- **For-loop over arrays**: only Range iterators (`start..end`, `start..=end`)
  are supported; arrays and other iterables produce a clear compile error

See `docs/develop/v0/stage-14/v0.1-capability-assessment.md` for the full
gap analysis and `docs/develop/v0/stage-15/v0.2-preparation.md` for the
v0.2 roadmap.

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
│   ├── ty/        MIR type system
│   └── drop_elaboration.rs  v0.2 Stage 15.43-15.61: Drop terminators
├── typeck/        Stage 2: type checking (unify + writeback)
├── borrowck/      Stage 2: borrow checking (NLL dataflow + Drop semantics)
├── codegen/       Stage 3: MIR → LLVM IR
│   ├── llvm/      LLVMSysEmitter (llvm-sys C API)
│   ├── text/      TextEmitter (LLVM IR as text)
│   └── trait_dispatch/  vtable + dynptr + orchestrator (§14.4 split)
├── driver.rs      Orchestrator: runs all stages
├── session.rs     Span, source map, diagnostic emitter
├── stdlib/        StdlibFacade (Rust-side type metadata)
└── bin/           CLI driver (landin-stage0 binary)
```

**Process compliance**: `docs/stage-committee-process.md` v3.23
- §13.4 design alignment
- §14.4 architectural split (files < 1000 LOC)
- §16 interface isolation (MIR-only codegen, no HIR lookup)
- §23 API naming standard (no glob re-exports, `<verb>_<noun>` patterns)
- §25 deep review (8 dimensions, including D8 pipeline test coverage)
- §29 inter-stage deep verification

---

## Documentation

- `docs/stage-committee-process.md` — process spec (v3.23)
- `docs/develop/v0/stage-14/` — Stage 14 dev logs + gate reviews + plan
- `docs/develop/v0/stage-15/` — Stage 15 (v0.2) dev logs + gate reviews
- `docs/develop/v0/stage-14/v0.1-capability-assessment.md` — v0.1 gap analysis
- `docs/develop/v0/stage-15/v0.2-preparation.md` — v0.2 roadmap
- `docs/lang-design/` — design documents (19-27: v0.2 design docs)
- `docs/tests/pipeline-test-coverage.md` — pipeline path coverage matrix
- `docs/tests/matrix.md` — test matrix per stage
- `docs/llvm/` — LLVM-specific docs (build setup, stage-specific changes)
- `docs/tools/debug/` — debug tool docs
- `RELEASE_NOTES.md` — per-version release notes
- `docs/worklog.md` — full worklog

---

## Test Counts (v0.187.0)

| Suite | Count | Pass rate |
|-------|-------|-----------|
| Rust lib tests | 221 | 100% |
| Rust integration tests | 2130 | 100% |
| Conformance tests (.lin) | 5216 | 100% |
| - Parse-only (`00-parse`) | 600 | 100% |
| - Typecheck (`01-typecheck`) | 1020 | 100% |
| - Borrowck (`02-borrowck`) | 815 | 100% |
| - Codegen (`03-codegen`) | 2275+ | 100% |
| - End-to-end run (`04-e2e/06-run-ok`) | 171 | 100% |
| - Soundness (`05-soundness`) | 500+ | 100% |
| - Stdlib (`06-stdlib`) | 502 | 100% |
| - Integration (`07-integration`) | 501 | 100% |
| Examples | 4 | 100% |
| Benchmarks | 5 | — |
| **Total** | **7567** | **100%** |

---

## v0.2 Roadmap

### Phase 1: Architectural Debt Payment ✅ COMPLETE
- Ty interning, SubstsRef, TraitResolver key, EmitValue handle, writeback consolidation

### Phase 2: Soundness Closures ✅ SUBSTANTIALLY COMPLETE
- Task 7 (HP-10): Fixpoint dataflow NLL — ✅ COMPLETE
- Task 8 (HP-12): Drop elaboration — ✅ COMPLETE
- Task 9 (HP-5): Region allocation — 🔧 Infrastructure integrated
- Task 10 (HP-3): Closure redesign — 🔧 Design only (deferred to v0.3)

### Phase 3: Feature Work (in progress)
- Task 11 (Monomorphization) — ⏳ Blocked (needs Task 3)
- Task 12 (Lifetime elision) — ⏳ Ready (next task)
- **Task 13 (impl Drop + RAII)** — **✅ COMPLETE** (Stage 15.61)
- Task 14 (Object safety) — ⏳ Blocked (needs Task 3)

### Phase 4: Polish + Incremental (future)
- Task 15 (Incremental compilation)
- Task 16 (HP-22: dyn_trait_calls into Terminator::Call)
- Task 17 (HP-13: Associated type normalization)
- Task 18 (HP-14: HRTB)
- Task 19 (For-loop over arrays)
- Task 20 (`Box<T>` in prelude)

---

## License

MIT (see `LICENSE`)

## Repository

https://github.com/redskaber/landin-lang

---

**Last updated**: 2026-08-02 (v0.196.0, Stage 15.71 — `impl Drop` + RAII COMPLETE)
