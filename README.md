# Landin

**Author**: redskaber

A work-in-progress systems programming language inspired by Rust, designed for
zero-cost abstractions, memory safety without garbage collection, and
predictable performance.

> **🎉 v0.1 RELEASE — Conformance gate reached: 5026/5000 tests (100.5%)!**
>
> **Status:** v0.22.1 — Stage 0-11 complete, Stage 12 ✅ COMPLETE (9/9), Stage 13 🔄 IN PROGRESS (13.1 ✅ TD-028, 13.2 ✅ TD-031 P0, 13.3 🔄 TD-030 prep done).
> **2258+ rust tests** + **5026 conformance tests** + 5 benchmarks. 0 clippy warnings.
> Process v3.21 (§0-§28). §16 interface isolation compliant (TD-028 CLOSED). §17.1/§17.2/§18.4 docs compliant.
> Cross-stage audit r216 (first-pass) + r217 (second-pass, 3 reports) + r219 (Stage 12 §25 deep review) + Stage 13.1-13.3 design alignment complete.
>
> **Milestones:**
> - Stage 0-4: ✅ Complete (lexer, parser, HIR, MIR, typeck, borrowck, codegen)
> - Stage 5: ✅ Complete (99 sub-stages — TraitResolver, vtable, dyn Trait, stdlib)
> - Stage 6: ✅ Complete (18 sub-stages — 47-module architecture, all files < 1500 LOC)
> - Stage 7: ✅ Complete (9 sub-stages — TD-015 region inference, TD-018 user-defined trait dyn)
> - Stage 8: ✅ Complete (7 sub-stages — v0.2 roadmap + §25.8 + §25 deep review + §17 docs standardization)
> - Stage 9: ✅ Complete (12 sub-stages — parse conformance 600/600)
> - Stage 10: ✅ Complete (8 sub-stages — CLI upgrade + all 8 conformance categories created)
> - Stage 11: ✅ Complete (10 sub-stages — conformance 1139→5026, v0.1 gate reached!)
> - Stage 12: ✅ COMPLETE (9/9 sub-stages — v0.1 release + r216+r217+r219 cross-stage audits + §25.8 backfill + plan-13 reframe + version revert + README corrections + final gate review + polish backfill)
> - Stage 13: 🔄 IN PROGRESS (13.1 ✅ TD-028 §16 CLOSED; 13.2 ✅ TD-031 P0 CLOSED — if-let/while-let; 13.3 🔄 TD-030 prep done; 13.3a-13.4 P0 pending)
>
> **Architecture:** 51+ modules. All files < 1500 LOC. Single responsibility per module.
> Data flows单向 (§16 compliant — TD-028 closed). Design docs synced (§25.8).
>
> **v0.1 gate:** conformance 5026/5000 ✅ — **GATE REACHED!** (ratified by r216 + r217 + r219 audits)
> **v0.3 prep:** 2 P0 blockers remaining (TD-030 closure call, TD-032 macro_rules!) — Stage 13.3a-13.4 target
> **v0.22.0 feature:** ✅ if-let / while-let (TD-031 P0 CLOSED) — first user-facing language feature of Stage 13

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

# Run conformance suite (5026 tests, auto-detect parse vs compile mode)
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
| 3 | `codegen/` | ✅ Complete | 309 |
| 4 | modules, closures, macros, benchmarks, ADR | ✅ Complete | 62 + 5 bench |
| 5 | `traits/`, vtable, dyn Trait, stdlib | ✅ Complete | 642 |
| 6 | architectural splits (47 modules) | ✅ Complete | — |
| 7 | region inference, user-defined trait dyn | ✅ Complete | 154 |
| 8 | v0.2 features + docs standardization | ✅ Complete | 38 |
| 9 | v0.1 parse conformance (600/600) | ✅ Complete | 600 conformance |
| 10 | CLI upgrade + all 8 conformance categories | ✅ Complete | +359 conformance |
| 11 | v0.1 full conformance (5026/5000) | ✅ Complete | +3887 conformance |

## v0.1 Conformance Gate — REACHED! 🎉

| Category | Required | Current | Status |
|----------|---------|---------|--------|
| 00-parse | 600 | 600 | ✅ 100% |
| 01-typecheck | 1000 | 1020 | ✅ 102% |
| 02-borrowck | 800 | 800 | ✅ 100% |
| 03-codegen | 600 | 601 | ✅ 100.2% |
| 04-e2e | 500 | 502 | ✅ 100.4% |
| 05-soundness | 500 | 500 | ✅ 100% |
| 06-stdlib | 500 | 502 | ✅ 100.4% |
| 07-integration | 500 | 501 | ✅ 100.2% |
| **Total** | **5000** | **5026** | **✅ 100.5%** |

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
├── Cargo.toml              v0.21.0 (autotests=false — single all_tests target)
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
│   ├── conformance/        .lin conformance suite + run_all.py (5026 tests — v0.1 gate reached!)
│   │   ├── 00-parse/       (600 tests — Stage 9, 100% ✅)
│   │   ├── 01-typecheck/   (1020 tests — Stage 10-11)
│   │   ├── 02-borrowck/    (800 tests — Stage 10-11)
│   │   ├── 03-codegen/     (601 tests — Stage 10-11)
│   │   ├── 04-e2e/         (502 tests — Stage 10-11)
│   │   ├── 05-soundness/   (500 tests — Stage 10-11)
│   │   ├── 06-stdlib/      (502 tests — Stage 10-11)
│   │   ├── 07-integration/ (501 tests — Stage 10-11)
│   │   └── run_all.py      Conformance runner (--mode auto/parse/compile)
│   └── v0/
│       ├── stage{0-9}/plan/  Stage 0-9 test files
│       ├── stage10/plan/     Stage 10 test files (independent directory)
│       └── stage11/plan/     Stage 11 test files (independent directory)
├── benches/                Performance benchmarks (5 benchmarks)
├── examples/               API demos + historical audit scripts
└── docs/
    ├── stage-committee-process.md  Process v3.21 (§13.4 + §14.4 + §25.8)
    ├── develop/v0/                 Dev logs + ADR + deep reviews + plans
    │   ├── stage-{0..9}/           Stage 0-9 dev logs + gate reviews + plans (§17.3)
    │   ├── stage-10/               Stage 10 dev logs + gate reviews + plans (independent)
    │   └── stage-11/               Stage 11 dev logs + gate reviews + plans (independent)
    ├── lang-design/                19 design docs (00-18) + CHANGELOG + FREEZE-REPORT
    ├── tests/                      Test plans + matrix
    │   └── v0/
    │       ├── stage{0..9}/        Stage 0-9 test plans (§17.2 双向印证)
    │       ├── stage10/            Stage 10 test plans (independent)
    │       └── stage11/            Stage 11 test plans (independent)
    └── worklog.md                  Worklog mirror (v3.18 §18.4.0) — synced through r215
```

## Testing

```bash
# Run all Rust tests
cargo test

# Run conformance suite (5026 tests, auto-detect parse vs compile mode)
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
- **Stage 9** ✅ COMPLETE (parse conformance 600/600)
- **Stage 10** ✅ COMPLETE (CLI upgrade + all 8 conformance categories created)
- **Stage 11** ✅ COMPLETE (conformance 1139→5026, v0.1 gate reached!)
- **Stage 12** ✅ COMPLETE (9/9 sub-stages — v0.1 release + r216 first-pass + r217 second-pass audits + §25.8 backfill + plan-13 reframe + version revert + README corrections + final gate review + polish backfill)
  - 12.1 ✅ v0.1 release + v0.3 bootstrap prep
  - 12.2 ✅ r216 first-pass audit (D1-D7, 5/5 GO-WITH-CONDITIONS)
  - 12.3 ✅ r217 second-pass audit (3 reports, 2055 lines, 9 stage-round revisions)
  - 12.4 ✅ §25.8 retroactive backfill (Stage 5 + Stage 8, 3 design-doc edits)
  - 12.5 ✅ plan-13.1.md reframed as Stage 12 output (Planned → Draft)
  - 12.6 ✅ Version revert v0.22.0 → v0.21.2 (patch bump, no new compiler features)
  - 12.7 ✅ Stage 0-4 README corrections (per r217 stages-0-4 findings)
  - 12.8 ✅ Final gate review (§25 deep review of Stage 12 — 5/5 GO-WITH-CONDITIONS-or-GO → PASS)
  - 12.9 ✅ Polish backfill (Stage 5 develop README + plan-6.{4,5,6}.md retroactive + v2.36 correction)
- **Stage 13** 🔄 IN PROGRESS (v0.3 self-hosting prep — TD-028..TD-033 closure)
  - 13.1 ✅ Architecture baseline — TD-028 §16 violation CLOSED (7 emit_* functions relocated mir→codegen)
  - 13.2 ✅ if-let / while-let — TD-031 P0 CLOSED (Strategy B desugar to Match; 11 conformance FAIL→PASS)
  - 13.3 🔄 Closure call lowering — TD-030 P0 PREPARATION DONE (§13.4 design alignment + blueprint; 13.3a implementation pending, HIGH risk ~600-1000 LOC)
  - 13.1b ⏳ TD-029 TyKind::Dynamic refactor (deferred per design alignment §15)
  - 13.4 ⏳ macro_rules! + 26 built-in macros (TD-032 — P0)
  - 13.5 ⏳ TD-033 P1 sub-items (for/move/HRTB/assoc-norm/two-phase/RFC 2229)
  - 13.6 ⏳ v0.1 release announcement (after P0 closure)
- **v0.1** = Stage 0 完整 + conformance 5026/5000 通过 ✅ **GATE REACHED!** (ratified by r216 + r217 + r219)
- **v0.3** = self-hosting (远期 — Stage 13 P0 closure in progress, 1/3 P0 closed, 1 in prep)

## Cross-stage audit (r216 + r217 + r219)

### r216 — first-pass audit (Stage 12.2)

Stage 12.2 triggered a multi-agent group review per §25 + §21 + §16 + §25.8:

- **ARCH-A** (D1 + D5): `docs/develop/v0/stage-12/cross-stage-audit-r216-architecture.md`
  - 1 active §16 violation (TD-028: `mir::dyn_trait → codegen`, ≤3 files to fix)
  - 1 newly-discovered B1 deviation (TD-029: `TyKind::Dynamic` missing in `src/mir/ty.rs`)
  - Verdict: GO-WITH-CONDITIONS
- **QA-A + REV-A + PM-A** (D2 + D3 + D4 + D6 + D7): `docs/develop/v0/stage-12/cross-stage-audit-r216-techdebt-tests-docs.md`
  - 7 open tech debt items (P0=3, P1=1, P2=2, P3=1-on-hold)
  - 7357 tests total (146 inline + 2179 integration + 5 benchmarks + 5026 conformance + 1 should_panic)
  - Stage 13 recommendation: Option B (compile pipeline fixes for v0.3 readiness)
  - Verdict: 5/5 GO-WITH-CONDITIONS

### r217 — second-pass audit (Stage 12.3, this stage)

Stage 12.3 triggered a second-pass review that revises r216 stage-round attributions and
re-audits Stages 0-11 systematically. 3 parallel subagent batches produced 2055 lines:

- **ARCH-A + REV-A** (stages 0-4): `cross-stage-audit-r217-stages-0-4.md` (411 lines)
  - 5 stage-round revisions (TD-028 attribution correct, TD-029 root cause reattributed to Stage 2.1, TD-030/031 numeric corrections, TD-032 framing inversion)
  - Stage 0-4 README per-module attribution errors identified (Stage 12.7 P2 fix)
- **ARCH-A + REV-A + QA-A** (stages 5-8): `cross-stage-audit-r217-stages-5-8.md` (671 lines)
  - 4 count corrections (Stage 5 has 96 plan files not 99; Stage 6 has 15 plans not 18)
  - 5 new findings (3 implicit-knowledge gaps in design docs: DynTraitMIRSummary, StdlibTypeKind, async/await MVP)
  - Stage 5 §25.8 retroactive backfill required (Stage 12.4 P1 fix)
- **PM-A + REC-A + ARCH-A** (stages 9-11 + Stage 12 scope): `cross-stage-audit-r217-stages-9-12-scope.md` (973 lines)
  - Stage 9-11 numeric claims all verified exact
  - Stage 12 scope finalized: 8 sub-stages (12.1-12.8), 12.8 final gate review pending
  - Stage 13 launch criteria: 5 conditions (all Stage 12.4-12.8 must close)
  - Version policy: v0.21.2 (patch bump, no new compiler features)

### §25.8 design write-backs

- Stage 12.2 (r216): `docs/lang-design/03-type-system.md` §13 — TyKind::Dynamic B1 deviation
- Stage 12.4 (r217 retroactive): `docs/lang-design/06-mir.md` §15 — DynTraitMIRSummary 4-layer arch
- Stage 12.4 (r217 retroactive): `docs/lang-design/09-stdlib.md` §12 — StdlibTypeKind converter
- Stage 12.4 (r217 retroactive): `docs/lang-design/05-ast.md` §15 — async/await MVP synchronous semantics

### r219 — Stage 12 §25 deep review (Stage 12.8 final gate review)

Stage 12.8 triggered the §25 seven-dimension deep review of Stage 12 itself (required before Stage 13 launch):

- **Full committee** (ARCH-A + QA-A + REV-A + PM-A + ALG-C + SKL-A):
  - `docs/develop/v0/stage-12/deep-review-stage12-r219.md` (514 lines, full §25 D1-D7 review)
  - `docs/develop/v0/stage-12/gate-review-12.8.md` (145 lines, concise gate summary)
- **Verdict**: 5/5 GO-WITH-CONDITIONS-or-GO → **PASS** (0 NO-GO)
- **Stage 12 closure**: ✅ **COMPLETE** (7/8 fully DONE + 1/8 PARTIAL with P2 follow-up; no P0/P1 blockers)
- **Stage 13 launch**: ✅ **AUTHORIZED** (4 GO + 1 GO-WITH-CONDITIONS on Stage 12.7 partial — non-blocking)

### Stage 12 sub-stage plan (final, per r219 + Stage 12.9)

| Sub-stage | Status | Description |
|-----------|--------|-------------|
| 12.1 | ✅ DONE | v0.1 release + v0.3 bootstrap prep |
| 12.2 | ✅ DONE | First-pass cross-stage audit r216 |
| 12.3 | ✅ DONE | Second-pass audit r217 (3 reports, 2055 lines) |
| 12.4 | ✅ DONE | §25.8 retroactive backfill (Stage 5 + Stage 8 — 3 design-doc edits) |
| 12.5 | ✅ DONE | Reframe plan-13.1.md as Stage 12 output (Planned → Draft) |
| 12.6 | ✅ DONE | Version revert v0.22.0 → v0.21.2 → v0.21.3 → v0.21.4 (Stage 12 closure + polish patch bumps) |
| 12.7 | ✅ DONE | Stage 0-4 README corrections (per r217 stages-0-4 findings) |
| 12.8 | ✅ DONE | Stage 12 final gate review (§25 deep review — 5/5 GO-WITH-CONDITIONS-or-GO → PASS) |
| 12.9 | ✅ DONE | Polish backfill (Stage 5 develop README + plan-6.{4,5,6}.md retroactive + v2.36 correction) |

### Stage 13 launch criteria (per r217 + r219)

Stage 13 launch criteria all closed:
1. Stage 12.4 §25.8 Stage 5 backfill complete ✅
2. Stage 12.5 plan-13.1.md reframed as Stage 12 output ✅
3. Stage 12.6 version revert done (Cargo.toml = v0.21.3) ✅
4. Stage 12.7 Stage 0-4 README corrections done ✅
5. Stage 12.8 final gate review 5/5 GO-WITH-CONDITIONS-or-GO ✅

**Stage 13**: ✅ AUTHORIZED to launch — Stage 13.1 may begin immediately with MUV-1 (TD-028 §16 fix, ≤3 files, ~4h)

## Documentation

- `docs/stage-committee-process.md` — Process SOP v3.21 (§1-§28)
- `docs/develop/v0/api-naming-standard.md` — API naming standard v2.37
- `docs/develop/v0/architecture-decisions.md` — 7 Architecture Decision Records
- `docs/develop/v0/stage-{0..9}/` — Stage 0-9 dev logs + gate reviews + plans (§17.3)
- `docs/develop/v0/stage-10/` — Stage 10 dev logs + gate reviews + plans (independent)
- `docs/develop/v0/stage-11/` — Stage 11 dev logs + gate reviews + plans (independent)
- `docs/develop/v0/stage-12/` — Stage 12 dev logs + v0.1 release + v0.3 prep + r216 first-pass + r217 second-pass audits
- `docs/develop/v0/stage-13/` — Stage 13 plan (DRAFT — Stage 12 output, awaits Stage 12 close)
- `docs/lang-design/` — 19 language design documents (v1.3.2 Final, frozen; §25.8 write-backs appended through Stage 12.4)
- `docs/tests/v0/stage{0..9}/` — Stage 0-9 test plans (§17.2 双向印证)
- `docs/tests/v0/stage10/` — Stage 10 test plans (independent)
- `docs/tests/v0/stage11/` — Stage 11 test plans (independent)
- `docs/tests/v0/stage12/` — Stage 12 test plans (independent)
- `docs/tests/v0/stage13/` — Stage 13 test plans (independent, planned)
- `docs/tests/matrix.md` — Global test matrix
- `docs/worklog.md` — Worklog mirror (v3.18 §18.4.0)

## License

MIT (see `LICENSE`).
