# Second-Pass Cross-Stage Audit (r217) — Stages 9-11 Re-audit + Stage 12 Scope Finalization

> **Auditor**: PM-A + REC-A + ARCH-A (combined for second-pass)
> **Date**: 2026-07-26
> **Baseline**: v0.21.0 → v0.21.2 (target patch bump; see §5.3 for rationale)
> **Scope**: Stages 9, 10, 11 re-audit + Stage 12 sub-stage plan finalization + Stage 13 launch criteria
> **Companion**:
> - `cross-stage-audit-r216-architecture.md` (350 lines, ARCH-A, D1+D5)
> - `cross-stage-audit-r216-techdebt-tests-docs.md` (650 lines, combined, D2+D3+D4+D6+D7)
> - `cross-stage-audit-r217-stages-0-4.md` (411 lines, ARCH-A + REV-A, second-pass for Stages 0-4)
> - `cross-stage-audit-r217-stages-5-8.md` (~360 lines, ARCH-A + REV-A + QA-A, second-pass for Stages 5-8)
> - This report: `cross-stage-audit-r217-stages-9-12-scope.md` (Stages 9-11 second-pass + Stage 12 scope finalization)

---

## 1. Executive Summary

The third second-pass audit (r217) covers the **three conformance-driven stages (9-11)**
that took the project from `v0.16.10` (pre-conformance) through `v0.20.0` (v0.1 release
gate reached: 5026/5000 conformance tests) and **finalizes Stage 12's proper scope**
per the user clarification that "we are at Stage 12 (just started), not Stage 13".

**Headline findings**:

- **3 stages re-audited** (Stages 9, 10, 11) covering **30 sub-stages** (12 + 9 + 10)
  and the conformance suite expansion from **8 → 5026 tests** (+62,725%).
- **0 count corrections** needed for Stages 9-11 (in contrast to the 4 corrections
  identified in r217-stages-5-8 and 5 TD numeric/framing errors in r217-stages-0-4).
  Every headline number — 600 parse tests, 145 Stage 9 rust tests, 1139 end-of-Stage-10
  conformance, 5026 end-of-Stage-11 conformance, per-category breakdown
  600/1020/800/601/502/500/502/501 — is **exactly as claimed**.
- **1 sub-stage count clarification** for Stage 10: there are **9** gate-review files
  (10.0 through 10.8), where 10.0 is an infra-prep stage (CLI upgrade + runner upgrade)
  and 10.1-10.8 are 8 conformance-category builds. The user's "8 sub-stages" framing
  refers to the 8 conformance stages; the committee's `gate-review-10.8.md` text
  "8/8 sub-stages (10.0-10.8)" is internally inconsistent (9 files listed, 8 counted).
- **Stage 12 scope finalized**: 8 sub-stages (12.1-12.8), with 12.1-12.3 = DONE/CURRENT
  and 12.4-12.8 = PENDING. The previous turn's `plan-13.1.md` was **premature** and
  must be reframed as a Stage 12 deliverable ("future-stage planning, awaiting
  Stage 12 close"), not a Stage 13 launch artifact.
- **5 Stage 13 launch criteria** defined (see §5.2): all of Stage 12.4-12.8 must close
  before Stage 13.1 implementation begins.
- **Version policy confirmed**: `v0.21.2` (patch bump, not `v0.22.0` minor bump).
  Stage 12 adds no new compiler features (only docs + audit + tests + plan-13.1
  reframe); per semver §2.0.0, patch is appropriate. `v0.22.0` reserved for Stage 13
  P0 closure (closures/if-let/macro_rules! — actual compiler features).
- **TD trajectory**: Stages 0-4 opened 16 TDs (TD-001..TD-016, all deferred to Stage 5+);
  Stage 5 opened 3 (TD-016-rep, TD-017, TD-018) + closed 2 (TD-014, TD-016-rep); Stage 6
  opened 7 (TD-019, TD-022..TD-027) + closed 8 (TD-011, TD-017, TD-022..TD-027, TD-027
  reverted); Stage 7 opened 0 + closed 2 (TD-015, TD-018); Stages 8-11 opened 0 + closed 0;
  Stage 12 opened 6 (TD-028..TD-033) + closed 0. **Net open at end of Stage 12.3: 7**
  (TD-019 user-hold + TD-028..TD-033).
- **§25.8 write-back coverage**: Stage 0-5 ❌ (process v3.20 or earlier); Stage 6 ✅ at
  6.18; Stage 7 ✅ at 7.7; Stage 8 ✅ at 8.6; Stage 9 ✅ at 9.12 (deep-review-r195
  §2.5 D5 + §2.7 D7 both reference §25.8 explicitly); Stage 10 ✅ at 10.8 (deep-review-r205
  §5.5 line 41: "19 份设计文档全部已通过 §25.8 同步"); Stage 11 ⚠️ partial — gate-review-11.10
  §25 deep review table cites "D5 Design | ✅ 19 docs synced, conformance as executable spec"
  but no separate deep-review-stage11-rXXX.md file exists; Stage 12 ✅ at 12.2 (r216 audit
  discovered TD-029 and wrote back to `03-type-system.md` §13).
- **Design-doc-vs-implementation delta trend**: Gap is **closing** but slowly. v0.1 baseline
  (end Stage 11): 18 B1 deviations identified (r216-architecture §3.1-§3.5). Of those:
  3 P0 + 6 P1 sub-items (9 total) scheduled for Stage 13 closure; remaining 9 are v0.2+
  long-term. r217 second-pass reattributed TD-029 root cause from Stage 5 to Stage 2.1
  (MIR types definition) — this **does not change the count** but tightens the closure
  scope (fix in `src/mir/ty.rs`, not `src/mir/dyn_trait.rs`).
- **Committee vote**: **GO-WITH-CONDITIONS** — Stage 12.3 (this audit) ratified; Stage
  13 launch NOT authorized until Stage 12.4-12.8 close the 5 launch criteria.

---

## 2. Stage 9 Re-audit

Stage 9 = **v0.1 conformance suite expansion (parse layer)**. Took conformance from
**8 → 600 tests** (+592, +7400%) across 12 sub-stages. Baseline `v0.16.10 → v0.17.0`.

### 2.1 Sub-stage count verification (12 sub-stages) ✅

Verified by `ls docs/develop/v0/stage-9/gate-review-9.{N}.md`:

```
gate-review-9.1.md   plan-9.1.md
gate-review-9.2.md   plan-9.2.md
gate-review-9.3.md   plan-9.3.md
gate-review-9.4.md   plan-9.4.md
gate-review-9.5.md   plan-9.5.md
gate-review-9.6.md   plan-9.6.md
gate-review-9.7.md   plan-9.7.md
gate-review-9.8.md   plan-9.8.md
gate-review-9.9.md   plan-9.9.md
gate-review-9.10.md  plan-9.10.md
gate-review-9.11.md  plan-9.11.md
gate-review-9.12.md  plan-9.12.md
```

**Count**: 12 gate-review files + 12 plan files + 1 deep-review (`deep-review-stage9-r195.md`)
+ 1 systematic review (`systematic-review-v0156.md`) = **26 stage-9 doc files**.

**Verdict**: ✅ 12 sub-stages confirmed (9.1 through 9.12), full plan + gate-review +
deep-review coverage. No plan/gate-review gaps (unlike Stages 5 and 6 which had gaps —
see r217-stages-5-8 §2.1 and §3.1).

### 2.2 Conformance count verification (600 parse tests) ✅

Verified by `find tests/conformance/00-parse/ -name "*.lin" | wc -l`:

- **600 `.lin` files** in `tests/conformance/00-parse/` — exact match to claim.
- Matches `deep-review-stage9-r195.md:78` table row "9.12 ✅ | 600 | 600 | 100% 🎉".
- Matches `gate-review-9.12.md:13` "python3 tests/conformance/run_all.py: 600 passed".

Per-category breakdown within `00-parse/` (verified by `find` per subdirectory):

| Sub-directory | Files | Topic |
|---------------|-------|-------|
| 00-literals | 33 | Literals + numeric forms |
| 01-operators | 60 | Operators + Pratt parsing |
| 02-control-flow | 80 | if/while/for/loop/break/continue (+ 11 if-let/while-let FAIL) |
| 03-patterns | 71 | Pattern syntax |
| 04-types | 60 | Type expressions |
| 05-attributes | 40 | Attribute syntax |
| 06-generics | 50 | Generic params + bounds |
| 07-closures | 40 | Closure syntax |
| 08-modules | 60 | mod/use/path |
| 09-error-recovery | 51 | Parser error recovery |
| 10-realistic | 55 | Realistic programs (+ 1 v0.1 milestone) |
| **Total** | **600** | ✅ exact |

**Verdict**: ✅ 600/600 conformance tests verified. v0.1 parse gate met.

### 2.3 Rust test integration verification (145 tests) ✅

Verified by `grep -c "^#\[test\]" tests/v0/stage9/plan/*.rs`:

| File | `#[test]` count |
|------|------------------|
| `attributes_tests.rs` | 10 |
| `closures_tests.rs` | 11 |
| `control_flow_tests.rs` | 14 |
| `deep_review_v01_rc_tests.rs` | 10 |
| `error_recovery_tests.rs` | 8 |
| `generics_tests.rs` | 10 |
| `modules_tests.rs` | 10 |
| `operators_tests.rs` | 11 |
| `patterns_tests.rs` | 16 |
| `realistic_programs_tests.rs` | 10 |
| `systematic_review_v0156_tests.rs` | 11 |
| `types_tests.rs` | 14 |
| `v0.1_gap_analysis_tests.rs` | 10 |
| **Total** | **145** ✅ |

Matches r216-techdebt line 213: `| stage9 (conformance expansion) | 13 | 145 |`.

**Verdict**: ✅ 145 rust tests across 13 files verified exactly. Stage 9 added 13 test
files totaling 145 tests covering the 11 conformance categories + 1 v0.1-RC deep-review
file + 1 v0.1 gap-analysis file.

### 2.4 Documentation file count verification (12 .md files) ✅

Verified by `ls docs/tests/v0/stage9/plan/`:

```
README.md
attributes.md
closures.md
control_flow.md
error_recovery.md
generics.md
modules.md
operators.md
patterns.md
realistic_programs.md
systematic_review_v0156.md
types.md
```

**Count**: 12 files (1 README + 11 topic .md files) — exact match to claim.

Cross-referenced against the 13 `.rs` test files in `tests/v0/stage9/plan/`: each topic
.md file in `docs/tests/v0/stage9/plan/` corresponds to a `.rs` test file of the same
name (minus `_tests` suffix), with the addition of `v0.1_gap_analysis.md` (matches
`v0.1_gap_analysis_tests.rs`) and `systematic_review_v0156.md` (matches
`systematic_review_v0156_tests.rs`). The `deep_review_v01_rc_tests.rs` does not have a
corresponding `.md` doc (the deep-review is captured in `docs/develop/v0/stage-9/deep-review-stage9-r195.md`
instead).

**Verdict**: ✅ 12 .md files verified. Slight asymmetry: 13 .rs test files but only 12 .md
docs (deep-review test file is documented in stage-9 dev directory, not tests directory).

### 2.5 §25.8 write-back verification (Stage 9.12) ✅

Read `docs/develop/v0/stage-9/deep-review-stage9-r195.md` (230 lines). Verified §25.8
discipline applied:

- **Line 118**: `**设计文档同步状态** (§25.8):` — D5 dimension header explicitly cites §25.8.
- **Lines 120-130**: 9-row design-doc sync table covering `01-language-specification.md` §13,
  `02-grammar.md` §5, `03-type-system.md` §10+§11+§12, `04-ownership-borrowing.md` §11+§12+§13,
  `05-ast.md` §13+§14, `06-mir.md` §14, `07-codegen.md` §14+§15, `09-stdlib.md` §11,
  `17-conformance-suite.md` §2 (600 tests) — all marked ✅.
- **Line 160**: `✅ process v3.21 (§13.4 + §14.4 + §25.8) 落地` — D7 dimension explicitly cites §25.8.
- **Line 169**: `**Q1 (设计对齐)**: ✅ 设计文档全部 synced (§25.8); conformance suite 作为可执行规范`
  — committee vote Q1 cites §25.8.
- **Line 132**: `**新发现偏差**: 0` — no new B1/B3 deviations introduced in Stage 9
  (conformance stages verify existing design; they don't introduce new design decisions).

`gate-review-9.12.md` (80 lines) confirms:
- Line 16: `## §25 深度审查` — section header.
- Line 18: reference to `deep-review-stage9-r195.md — 5/5 GO → PASS`.
- Line 25: `D5 设计对齐 | ✅ 8 core design docs synced; conformance suite as executable spec`.
- Line 62: `**5/5 GO → PASS** — **v0.1 release candidate 宣布!**`

**Verdict**: ✅ Stage 9.12 §25.8 write-back verified — design-doc sync table present,
all 9 core design docs marked synced, 0 new deviations, 5/5 GO → PASS committee vote.
Stage 9 conformance stages do **not** introduce new B1/B3 deviations (conformance is
executable spec, not new design), so §25.8 is applied as design-doc sync verification
rather than deviation write-back.

---

## 3. Stage 10 Re-audit

Stage 10 = **CLI upgrade + 8 conformance categories creation + initial batches**. Took
conformance from **600 → 1139 tests** (+539, +89.8%) across 9 sub-stages (10.0 + 10.1-10.8).
Baseline `v0.17.1 → v0.18.0`.

### 3.1 Sub-stage count verification (9 files; 8 conformance + 1 infra) ⚠️

Verified by `ls docs/develop/v0/stage-10/gate-review-10.*.md | sort -V`:

```
gate-review-10.0.md   (infra prep: CLI upgrade + Runner upgrade, v0.17.1 → v0.17.2)
gate-review-10.1.md   (00-parse already at 600; no new conformance here)
gate-review-10.2.md   (01-typecheck initial batch — 120 tests)
gate-review-10.3.md   (02-borrowck initial batch — 80 tests)
gate-review-10.4.md   (03-codegen initial batch — 61 tests)
gate-review-10.5.md   (04-e2e + 05-soundness + 06-stdlib initial batches)
gate-review-10.6.md   (07-integration — cross-module + multi-crate + feature-gate)
gate-review-10.7.md   (07-integration final batch — 1059 cumulative)
gate-review-10.8.md   (typecheck expansion +80; 1139 cumulative + §25 deep review)
```

**File count**: 9 gate-review files + 9 plan files (`plan-10.0.md` through `plan-10.8.md`)
+ 1 deep-review (`deep-review-stage10-r205.md`) + 1 plan-stage10 (`plan-stage10.md`) +
1 v0.1-gap-analysis (`v0.1-gap-analysis.md`) + 1 gate-review-v0.1-gap
(`gate-review-v0.1-gap.md`) = **23 stage-10 doc files**.

**Counting ambiguity**: The user task says "8 sub-stages"; the actual file count is 9
(10.0 through 10.8). `gate-review-10.8.md:36` says "8/8 sub-stages (10.0-10.8) complete"
— this is internally inconsistent: 9 files are listed in parentheses, but the count says
"8/8". The most coherent interpretation:
- **10.0** = infra-prep stage (Format migration + CLI upgrade + Runner upgrade)
- **10.1-10.8** = 8 conformance-build sub-stages (one per category, with categories
  sometimes split across multiple sub-stages — e.g., 07-integration spans 10.6 + 10.7)
- "8/8" in gate-review-10.8 likely refers to "8 conformance sub-stages complete" (10.1-10.8),
  treating 10.0 as a prep stage not counted in the conformance work tally.

**Verdict**: ⚠️ Sub-stage count is **9 files (10.0-10.8)** but **8 conformance-build
sub-stages (10.1-10.8)** — the user's "8 sub-stages" framing matches the conformance
interpretation; the committee's "8/8 (10.0-10.8)" wording is internally inconsistent and
should be corrected to either "8/8 (10.1-10.8)" or "9/9 (10.0-10.8)" in any future
revision of `gate-review-10.8.md`. Recommend Stage 12.4 errata note.

### 3.2 CLI features verification (all 4 flags present) ✅

Read `src/bin/main.rs` (114 lines). Verified all 4 CLI flags:

| Flag | Definition line | Behavior | Status |
|------|------------------|----------|--------|
| `--emit-tokens` | line 17 (`#[arg(long)] emit_tokens: bool`) | Lexes input + prints `tok.kind` for each token; returns early (lines 53-58) | ✅ |
| `--emit-ast` | line 21 (`#[arg(long)] emit_ast: bool`) | Parses input + prints `Crate with N items` summary + per-item `item.kind` debug (lines 69-74) | ✅ |
| `--compile` | line 26 (`#[arg(long)] compile: bool`) | Full pipeline via `driver::compile(&source_file.src)`; exits 1 on `result.has_errors()` (lines 86-98) | ✅ |
| `--emit-llvm-ir` | line 30 (`#[arg(long)] emit_llvm_ir: bool`) | Implies `--compile`; after success, calls `codegen::codegen_crate(&result)` and prints LLVM IR (lines 101-103) | ✅ |

`Cli` struct uses `clap::Parser` derive (line 8) with `#[command(name = "landin-stage0", version, about = "Landin compiler (stage 0)")]` (line 9). The `version` attribute reads from `Cargo.toml` `version` field automatically.

**Error handling**:
- File read error → `eprintln!("error: cannot read file {}: {e}", cli.file.display())` + exit 2 (lines 36-42)
- Lex/parse errors → eprintln per error + exit 1 (lines 76-83)
- Compile errors → `result.errors.format_for_user(...)` + exit 1 (lines 89-98)
- Success → eprintln "info: successfully parsed N items" (line 113) or "info: successfully compiled N items" (line 106)

**Verdict**: ✅ All 4 CLI flags present and wired to behavior. Matches `README.md:41-48`
quick-start examples. The flags map directly to the conformance runner's `--mode` choices
(see §3.4).

### 3.3 Conformance category verification (8 categories exist) ✅

Verified by `ls tests/conformance/`:

```
00-parse        (Stage 9: 600 tests)
01-typecheck    (Stage 10.2: 120 → Stage 10.8: 200 → Stage 11: 1020)
02-borrowck     (Stage 10.3: 80 → Stage 11: 800)
03-codegen      (Stage 10.4: 61 → Stage 11: 601)
04-e2e          (Stage 10.5: 48 → Stage 11: 502)
05-soundness    (Stage 10.5: 50 → Stage 11: 500)
06-stdlib       (Stage 10.5: 50 → Stage 11: 502)
07-integration  (Stage 10.6+10.7: 50 → Stage 11: 501)
README.md       (conformance suite overview)
run_all.py      (runner with --mode auto)
```

**Count**: 8 category directories + 1 README + 1 runner = 10 entries.

Each category directory has sub-directories by topic (e.g., `01-typecheck/02-generics/`,
`07-integration/00-multi-crate/`, `07-integration/01-cross-module/`,
`07-integration/02-feature-gate/`).

**Verdict**: ✅ All 8 conformance categories exist with sub-directory structure. Matches
`gate-review-10.8.md:37` "All 8 conformance categories created with initial batch".

### 3.4 Runner `--mode auto` verification ✅

Read `tests/conformance/run_all.py` (262 lines). Verified `--mode auto` support:

- **Line 127**: `def run_test(test: ConformanceTest, binary: Path, verbose: bool, mode: str = "auto") -> tuple[bool, str]:`
- **Lines 130-133** (docstring):
  ```
  mode="parse": uses --emit-ast (legacy, only validates parse stage)
  mode="compile": uses --compile (validates full pipeline)
  mode="auto": auto-detect based on test path — 00-parse uses parse mode,
               everything else (01-typecheck, 02-borrowck, etc.) uses compile mode
  ```
- **Lines 135-140** (auto-detect logic):
  ```python
  if mode == "auto":
      # Auto-detect: tests under 00-parse/ use parse mode, everything else uses compile
      if "00-parse" in str(test.path):
          mode = "parse"
      else:
          mode = "compile"
  ```
- **Lines 207-210** (argparse):
  ```python
  parser.add_argument(
      "--mode",
      choices=["auto", "parse", "compile"],
      default="auto",
      help="Test mode: auto (default, auto-detect per test), parse (--emit-ast), or compile (--compile)",
  )
  ```
- **Line 230**: `print(f"Running {len(tests)} conformance tests against {args.binary} (mode={args.mode})")`
- **Line 238**: `ok, msg = run_test(test, args.binary, args.verbose, args.mode)`

**Verdict**: ✅ `--mode auto` is the **default** (no flag needed); auto-detect routes
`00-parse/` tests to `--emit-ast` (parse mode) and all other categories to `--compile`
(full pipeline mode). This is the correct design: parse tests verify Stage 0 lexer/parser
only; typecheck/borrowck/codegen/e2e/soundness/stdlib/integration tests verify the full
compile pipeline.

### 3.5 Initial conformance count verification (1139 at end of Stage 10) ✅

Verified across multiple Stage 10 docs:

| Source | Claim | Verified |
|--------|-------|----------|
| `gate-review-10.7.md:11` | `python3 tests/conformance/run_all.py: 1059 passed, 0 failed` | ✅ Stage 10.7 ended at 1059 |
| `gate-review-10.7.md:14` | `🎉 All 8 conformance categories now exist!` | ✅ All 8 created by 10.7 |
| `gate-review-10.8.md:11` | `python3 tests/conformance/run_all.py: 1139 passed, 0 failed` | ✅ Stage 10.8 ended at 1139 (+80 typecheck expansion) |
| `gate-review-10.8.md:30` | `Total | 5000 | 1139 | 22.8%` | ✅ 22.8% of v0.1 gate (5000) |
| `gate-review-10.8.md:39` | `v0.1 progress: 600 → 1139 (+539, +89.8%)` | ✅ Stage 10 contribution: +539 |
| `deep-review-stage10-r205.md:6` | `2290 rust tests + 1139 conformance tests + 5 benchmarks` | ✅ Match |
| `deep-review-stage10-r205.md:13` | `Conformance 从 600 → 1139 (22.8% of v0.1 gate)` | ✅ Match |
| `deep-review-stage10-r205.md:69` | `Total | 5000 | 1139 | 22.8%` | ✅ Match |

**Stage 10 per-category breakdown at end of 10.8** (from `gate-review-10.8.md:22-30`):

| Category | Required (v0.1 gate) | End Stage 10 | % |
|----------|---------------------|--------------|---|
| 00-parse | 600 | 600 | 100% ✅ |
| 01-typecheck | 1000 | 200 | 20% |
| 02-borrowck | 800 | 80 | 10% |
| 03-codegen | 600 | 61 | 10.2% |
| 04-e2e | 500 | 48 | 9.6% |
| 05-soundness | 500 | 50 | 10% |
| 06-stdlib | 500 | 50 | 10% |
| 07-integration | 500 | 50 | 10% |
| **Total** | **5000** | **1139** | **22.8%** |

**Verdict**: ✅ 1139 conformance tests at end of Stage 10 verified exactly. Matches
r216-techdebt line 213 claim. Stage 10 created the 8-category skeleton + 8 initial batches
(~50-200 tests per category) + 1 typecheck expansion demonstration (+80). Stage 11 then
filled each category to its required target.

---

## 4. Stage 11 Re-audit

Stage 11 = **per-category conformance expansion to v0.1 gate target**. Took conformance
from **1139 → 5026 tests** (+3887, +341%) across 10 sub-stages. Baseline
`v0.18.7 → v0.19.0 → v0.20.0` (final v0.1 gate reached at 11.9; 11.10 = §25 deep review +
README rewrite).

### 4.1 Sub-stage count verification (10 sub-stages) ✅

Verified by `ls docs/develop/v0/stage-11/gate-review-11.*.md | sort -V`:

```
gate-review-11.1.md   plan-11.1.md
gate-review-11.2.md   plan-11.2.md
gate-review-11.3.md   plan-11.3.md
gate-review-11.4.md   plan-11.4.md
gate-review-11.5.md   plan-11.5.md
gate-review-11.6.md   plan-11.6.md
gate-review-11.7.md   plan-11.7.md
gate-review-11.8.md   plan-11.8.md
gate-review-11.9.md   plan-11.9.md
gate-review-11.10.md  plan-11.10.md
```

**Count**: 10 gate-review files + 10 plan files + 1 README = **21 stage-11 doc files**.

**Note**: Unlike Stages 5, 6, 9, and 10, Stage 11 has **no separate deep-review-stage11-rXXX.md
file**. The §25 deep review is folded into `gate-review-11.10.md` (lines 13-23 contain the
§25 deep review table with D1-D7 dimensions). This is a process-compliance variance from
Stages 6-10 (each had a dedicated `deep-review-stageN-rXXX.md`); should be noted in any
Stage 12.4 process audit.

**Verdict**: ✅ 10 sub-stages confirmed (11.1 through 11.10), full plan + gate-review
coverage. Note: deep-review folded into 11.10 (no separate deep-review-stage11-rXXX.md).

### 4.2 Conformance total verification (5026) ✅

Verified by `find tests/conformance/ -name "*.lin" -type f | wc -l`:

- **5026 `.lin` files** total — exact match to claim.
- Matches `gate-review-11.10.md:10`: `python3 tests/conformance/run_all.py: 5026 passed, 0 failed`.
- Matches `gate-review-11.9.md:10` (the moment v0.1 gate was first reached):
  `python3 tests/conformance/run_all.py: 5026 passed, 0 failed`.
- Matches `gate-review-11.9.md:25`: `Total | 5000 | 5026 | 100.5% ✅` (gate exceeded by 26 tests).
- Matches `README.md:9`: `🎉 v0.1 RELEASE — Conformance gate reached: 5026/5000 tests (100.5%)!`.

**Verdict**: ✅ 5026 conformance tests verified exactly. v0.1 gate reached (100.5% of
5000-test target).

### 4.3 Per-category breakdown verification ✅

Verified by running `find tests/conformance/{NN-*}/* -name "*.lin" | wc -l` for each
category:

| Category | Required | Actual | % of required | Match to r216/claim |
|----------|---------|--------|---------------|---------------------|
| 00-parse | 600 | **600** | 100.0% | ✅ Exact |
| 01-typecheck | 1000 | **1020** | 102.0% | ✅ Exact (+20 over target) |
| 02-borrowck | 800 | **800** | 100.0% | ✅ Exact |
| 03-codegen | 600 | **601** | 100.2% | ✅ Exact (+1 over target) |
| 04-e2e | 500 | **502** | 100.4% | ✅ Exact (+2 over target) |
| 05-soundness | 500 | **500** | 100.0% | ✅ Exact |
| 06-stdlib | 500 | **502** | 100.4% | ✅ Exact (+2 over target) |
| 07-integration | 500 | **501** | 100.2% | ✅ Exact (+1 over target) |
| **Total** | **5000** | **5026** | **100.5%** | ✅ Exact (+26 over target) |

Sum verified: 600 + 1020 + 800 + 601 + 502 + 500 + 502 + 501 = **5026** ✅.

**Verdict**: ✅ All 8 per-category counts verified exactly. v0.1 gate met or exceeded in
every category; 7 of 8 categories exceeded target (only `00-parse` and `05-soundness` at
exactly target; `01-typecheck` has the largest surplus at +20).

### 4.4 v0.1 gate ratification verification (Stage 11.10 §25 deep review) ✅

Read `docs/develop/v0/stage-11/gate-review-11.10.md` (39 lines). Verified:

- **Line 1**: `# Stage 11.10 Gate Review — §25 deep review + v0.1 release prep + README rewrite`
- **Line 3**: `版本: v0.19.0 → v0.20.0 | 流程: §25 + §13.4 + §17.1/§17.2/§17.3 + §1.2`
- **Lines 7-11** (CI/CD):
  ```
  cargo test: 2316 passed (146 unit + 2170 integration, 0 failed, 2 ignored)
  cargo fmt --check: clean
  cargo clippy --all-targets: 0 warnings, 0 errors
  python3 tests/conformance/run_all.py: 5026 passed, 0 failed
  ```
- **Line 13**: `## §25 deep review: 5/5 GO → PASS`
- **Lines 15-23** (7-dimension table):

  | Dimension | Status |
  |-----------|--------|
  | D1 Architecture | ✅ 50+ modules, all < 1500 LOC, 3 independent stage dirs |
  | D2 Tech Debt | ✅ TD-019 OPEN (user hold), ~300+ limitations documented |
  | D3 Tests | ✅ 2314 rust + 5026 conformance (100.5% of v0.1 gate) |
  | D4 v0.1 Readiness | ✅ GATE REACHED — 5026/5000 |
  | D5 Design | ✅ 19 docs synced, conformance as executable spec |
  | D6 Performance | ✅ 5026 tests in ~3 sec, no O(n²) |
  | D7 Docs | ✅ §17 fully compliant, README completely rewritten |

- **Line 25**: `## v0.1 gate status: REACHED! 🎉`
- **Line 27**: `## 委员会投票: 5/5 GO → PASS`
- **Lines 29-32**: `Stage 11 complete! 10/10 sub-stages (11.1-11.10); Conformance: 1139 → 5026 (+3887, +341%); v0.1 gate: 5026/5000 ✅`

**Verdict**: ✅ v0.1 gate ratified at Stage 11.10 with **5/5 GO → PASS** committee vote.
All 7 dimensions (D1-D7) marked ✅. Note: D1 says "50+ modules, all < 1500 LOC" which
the r217-stages-5-8 audit §3.5 clarified as "15 module directories / 90 source files / 7
large files < 1500 LOC" — the "50+ modules" framing is imprecise but the underlying claim
(no file exceeds 1500 LOC) is correct.

### 4.5 README rewrite verification ✅

Read `README.md` (274 lines) top section. Verified v0.1 gate messaging:

- **Line 1**: `# Landin`
- **Line 3**: `**Author**: redskaber`
- **Lines 5-7**: Project description (systems programming language inspired by Rust)
- **Line 9**: `> **🎉 v0.1 RELEASE — Conformance gate reached: 5026/5000 tests (100.5%)!**`
- **Line 11**: `> **Status:** v0.22.0 — Stage 0-12 complete + Stage 13.1 planned. v0.1 conformance gate ratified.` ← ⚠️ **VERSION INCORRECT** — should be `v0.21.2` per user clarification (see §5.3)
- **Line 12**: `> **2325+ rust tests** + **5026 conformance tests** + 5 benchmarks. 0 clippy warnings.`
- **Line 14**: `> Cross-stage audit r216 complete (D1-D7, 5/5 GO-WITH-CONDITIONS).` ← should mention r217 second-pass audits
- **Lines 16-26**: Milestones table (Stages 0-12 listed; Stage 13 marked "🔄 Planned")
- **Line 23**: `Stage 10: ✅ Complete (8 sub-stages — CLI upgrade + all 8 conformance categories created)` ← matches §3.1 finding
- **Line 24**: `Stage 11: ✅ Complete (10 sub-stages — conformance 1139→5026, v0.1 gate reached!)`
- **Line 25**: `Stage 12: ✅ Complete (Stage 12.1 — v0.1 release + v0.3 bootstrap prep; Stage 12.2 — cross-stage audit r216 + Stage 13 plan ratification)` ← ⚠️ **PREMATURE** — Stage 12 is NOT complete (only 12.1 + 12.2 done; 12.3-12.8 pending)
- **Line 26**: `Stage 13: 🔄 Planned (v0.3 self-hosting preparation — TD-028..TD-033 closure per r216 audit)` ← should be reframed as "📋 Draft (Stage 12 output, awaiting Stage 12 close)" per §5.2
- **Line 31**: `> **v0.1 gate:** conformance 5026/5000 ✅ — **GATE REACHED!** (ratified by r216 audit)` ← should also mention r217 second-pass
- **Line 32**: `> **v0.3 prep:** 3 P0 blockers (TD-030 closure call, TD-031 if-let, TD-032 macro_rules!) — Stage 13 target`

**Verdict**: ✅ README contains v0.1 gate reached messaging at the top (line 9). ⚠️ Three
corrections needed in Stage 12.6 (version sync):
1. Line 11: `v0.22.0` → `v0.21.2`
2. Line 25: `Stage 12: ✅ Complete` → `Stage 12: 🔄 In Progress (12.1-12.3 done; 12.4-12.8 pending)`
3. Line 26: `Stage 13: 🔄 Planned` → `Stage 13: 📋 Draft (Stage 12 output, awaiting Stage 12 close)`

Also need to update line 14 + line 31 to mention r217 second-pass audits alongside r216.

---

## 5. Stage 12 Scope Finalization (CRITICAL)

Per user clarification: **"We are at Stage 12 (just started) — NOT Stage 13 yet"**. The
previous turn's `plan-13.1.md` was premature and must be reframed. This section finalizes
Stage 12's proper scope with 8 sub-stages.

### 5.1 Stage 12 sub-stage plan (revised)

| Sub-stage | Topic | Status | Description | Deliverable |
|-----------|-------|--------|-------------|-------------|
| **12.1** | v0.1 release + v0.3 bootstrap prep | ✅ **DONE** | Shipped v0.1 release notes + v0.3 multi-month rewrite plan; baseline `v0.21.0` | `v0.1-release.md` (4 KB) + `v0.3-bootstrap-prep.md` (3 KB) + `plan-12.1.md` (1 KB) + `gate-review-12.1.md` (542 B, 5/5 GO → PASS) |
| **12.2** | First-pass cross-stage audit r216 | ✅ **DONE** | D1+D5 architecture audit + D2-D7 techdebt/tests/docs audit; discovered TD-028..TD-033; wrote back TD-029 to `03-type-system.md` §13 | `cross-stage-audit-r216-architecture.md` (350 lines, GO-WITH-CONDITIONS) + `cross-stage-audit-r216-techdebt-tests-docs.md` (650 lines, GO-WITH-CONDITIONS) |
| **12.3** | Second-pass cross-stage audit r217 (3-part) | 🔄 **CURRENT** (this audit closes part 3) | Stage round revision + Stages 0-4 re-audit (part 1) + Stages 5-8 re-audit (part 2) + Stages 9-11 re-audit + Stage 12 scope finalization (part 3, **this report**) | `cross-stage-audit-r217-stages-0-4.md` (411 lines, GO-WITH-CONDITIONS) + `cross-stage-audit-r217-stages-5-8.md` (~360 lines, GO-WITH-CONDITIONS) + `cross-stage-audit-r217-stages-9-12-scope.md` (this report) |
| **12.4** | §25.8 retroactive backfill for Stage 5 | ⏳ **PENDING** | Stage 5 ran on process v3.20 (pre-§25.8) and never had retroactive §25.8 write-back for its 3 implicit-knowledge items (DynTraitMIRSummary, StdlibTypeKind converter, stdlib semantic grouping) | Edits to `06-mir.md` §6 (add DynTraitMIRSummary) + `03-type-system.md` §2 or `09-stdlib.md` (add StdlibTypeKind + stdlib_type_kind_to_emit_type) |
| **12.5** | Stage 13 plan reframe | ⏳ **PENDING** | Move `plan-13.1.md` content to Stage 12 output framing; mark as "future-stage planning, awaiting Stage 12 close"; update header from `🔄 Planned` to `📋 Draft (Stage 12 output, awaiting Stage 12 close)`; cross-reference from `stage-12/README.md` | Updated `plan-13.1.md` header + new cross-reference entry in `stage-12/README.md` |
| **12.6** | Cargo.toml version revert + sync | ⏳ **PENDING** | Revert `version = "0.22.0"` → `version = "0.21.2"`; sync README.md status line + RELEASE_NOTES.md + api-naming-standard.md | Updated `Cargo.toml` + `README.md` + `RELEASE_NOTES.md` + `api-naming-standard.md` |
| **12.7** | Stage 0-4 README corrections | ⏳ **PENDING** | Per r217-stages-0-4 §3 finding: 4 of 5 Stage 0-4 README files have incorrect per-module test breakdowns (totals are correct); fix `stage4/plan/README.md` reference to nonexistent `module_tests.rs` → `visibility_tests.rs`; correct ast_structure off-by-1 in stage0; correct 3 of 4 module counts in stage1; correct all 4 module counts in stage2; add missing `deep_inspection_tests.rs` entry in stage3 | Updated 5 README files in `docs/tests/v0/stage{0..4}/plan/README.md` |
| **12.8** | Stage 12 final gate review | ⏳ **PENDING** | §25 deep review of Stage 12 itself (D1-D7 dimensions) + Stage 12 close + explicit Stage 13 launch authorization (if all 5 launch criteria met) | `gate-review-12.8.md` + `deep-review-stage12-rXXX.md` |

**Sub-stage count**: 8 (12.1 through 12.8). Status breakdown: 2 DONE (12.1, 12.2) + 1
CURRENT (12.3, this audit) + 5 PENDING (12.4, 12.5, 12.6, 12.7, 12.8).

**Stage 12 exit criteria** (proposed):
1. ✅ Stage 12.1 v0.1 release shipped (DONE)
2. ✅ Stage 12.2 r216 first-pass audit complete (DONE)
3. ✅ Stage 12.3 r217 second-pass audit complete (3 parts, this report closes part 3) (CURRENT → DONE at this commit)
4. ⏳ Stage 12.4 §25.8 Stage 5 backfill complete (3 design-doc edits)
5. ⏳ Stage 12.5 `plan-13.1.md` reframed as Stage 12 output (header + cross-reference)
6. ⏳ Stage 12.6 Cargo.toml version reverted to v0.21.2 + README/RELEASE_NOTES/api-naming-standard synced
7. ⏳ Stage 12.7 Stage 0-4 README per-module test counts corrected (5 README files)
8. ⏳ Stage 12.8 §25 deep review of Stage 12 + Stage 13 launch decision (gate-review-12.8.md + deep-review-stage12-rXXX.md)

### 5.2 Stage 13 launch criteria (NOT Stage 13 launch)

The previous turn's `plan-13.1.md` (located at `docs/develop/v0/stage-13/plan-13.1.md`)
was created with header `Status: 🔄 Planned (per §13.4 design alignment)` and version
target `v0.21.0 → v0.22.0`. Both of these are **premature**:

- **Status `🔄 Planned`** implies Stage 13 is launched and execution is pending; in
  reality Stage 12 has not closed, so Stage 13 cannot be "Planned" — it can only be
  "Draft" (planning artifact produced by Stage 12, awaiting Stage 12 close).
- **Version `v0.22.0`** implies a minor-version bump for Stage 13 launch; in reality
  Stage 12 itself should be `v0.21.2` (patch — see §5.3), and Stage 13's eventual
  feature work (closures/if-let/macro_rules!) will warrant the `v0.22.0` minor bump
  when those features actually ship.

**Stage 13 launch criteria** (all 5 must be met before Stage 13.1 implementation begins):

| # | Criterion | Verification | Status |
|---|-----------|--------------|--------|
| 1 | Stage 12.4 §25.8 Stage 5 backfill complete | `06-mir.md` §6 contains `DynTraitMIRSummary`; `03-type-system.md` §2 or `09-stdlib.md` contains `StdlibTypeKind` + `stdlib_type_kind_to_emit_type()` | ⏳ Pending |
| 2 | Stage 12.5 `plan-13.1.md` reframed as Stage 12 output | `plan-13.1.md` header reads `📋 Draft (Stage 12 output, awaiting Stage 12 close)` (not `🔄 Planned`); `stage-12/README.md` cross-references the plan as a Stage 12 deliverable | ⏳ Pending |
| 3 | Stage 12.6 version revert done | `Cargo.toml` `version = "0.21.2"`; `README.md:11` reads `v0.21.2`; `RELEASE_NOTES.md` has v0.21.2 entry; `api-naming-standard.md` stage-12 section reflects v0.21.2 | ⏳ Pending |
| 4 | Stage 12.7 README corrections done | 5 stage-N plan/README.md files in `docs/tests/v0/stage{0..4}/plan/` have per-module test counts matching actual `#[test]` counts in `tests/v0/stage{0..4}/plan/*.rs`; nonexistent `module_tests.rs` reference in stage4 README replaced with `visibility_tests.rs` | ⏳ Pending |
| 5 | Stage 12.8 final gate review GO | `gate-review-12.8.md` produced with 5/5 GO → PASS; `deep-review-stage12-rXXX.md` produced with D1-D7 all ✅; explicit "Stage 13 launch authorized" line in gate-review-12.8.md | ⏳ Pending |

**Stage 13 launch authorization**: **NOT GRANTED** as of end of Stage 12.3. All 5 criteria
must close in Stage 12.4-12.8 before Stage 13.1 implementation begins. This is consistent
with r217-stages-0-4 §6 and r217-stages-5-8 §8 recommendations.

**Reframe action for `plan-13.1.md`** (Stage 12.5 scope):
1. Update header line 3 from `> **版本**: v0.21.0 → v0.22.0 (target) | **状态**: 🔄 Planned`
   to `> **版本**: v0.21.2 (Stage 12 baseline) → v0.22.0 (Stage 13 target, contingent on Stage 12 close) | **状态**: 📋 Draft (Stage 12 output, awaiting Stage 12 close)`
2. Add a top-of-file banner:
   ```
   > ⚠️ **STAGE 13 NOT YET LAUNCHED**
   >
   > This document is a Stage 12 deliverable (future-stage planning). Stage 13.1
   > implementation cannot begin until Stage 12.4-12.8 close the 5 launch criteria
   > listed in `cross-stage-audit-r217-stages-9-12-scope.md` §5.2.
   ```
3. Cross-reference from `docs/develop/v0/stage-12/README.md` as a Stage 12.5 deliverable
4. Update `Task ID:` references in the plan from `stage13.1-muv*-r217` to
   `stage12.5-plan13-draft-r217` (since r217 is the audit round, not the implementation
   round — implementation round will be r218+ when Stage 13 actually launches)

### 5.3 Version policy confirmation (v0.21.2, NOT v0.22.0)

**Current state**: `Cargo.toml` line 3 reads `version = "0.22.0"` (bumped from `v0.21.0`
in the previous turn, treating Stage 12.2 as a minor-version release).

**User request**: `v0.21.2` (patch bump).

**Semver analysis** (per [semver §2.0.0](https://semver.org/)):

| Bump type | Version | Justification | Applicable? |
|-----------|---------|---------------|-------------|
| **Patch (0.21.x)** | **v0.21.2** | Bug fixes / metadata / docs only — no new compiler features, no API changes | ✅ **YES** — Stage 12.2/12.3 added only docs (audit reports + plan + READMEs) and 10 verification tests; zero compiler behavior changes |
| Minor (0.x.0) | v0.22.0 | New features / API additions — backward-compatible | ❌ NO — no new compiler features; the Cargo.toml `description` field was expanded (Stage 5.1-7.8 history appended) but that's metadata, not functionality |
| Major (x.0.0) | v1.0.0 | Breaking changes | ❌ NO — no breaking changes |

**Semver §2.0.0 direct quote**: *"Patch version Z (x.y.Z | x > 0) MUST be incremented
when only backwards compatible bug fixes are introduced. A bug fix is defined as an
internal change that fixes incorrect behavior."*

**Stage 12 work characterization**:
- Stage 12.1: docs only (`v0.1-release.md` + `v0.3-bootstrap-prep.md`)
- Stage 12.2: docs only (2 audit reports, 350 + 650 lines)
- Stage 12.3: docs only (3 second-pass audit reports, ~411 + 360 + this report lines)
- Stage 12.4-12.7: docs only (design-doc edits + plan reframe + version sync + README corrections)
- Stage 12.8: docs only (gate review + deep review)
- **Zero** `src/` code changes across Stage 12 (except potentially TD-029 §25.8 write-back
  which is a design-doc edit, not a code edit)

**Verdict**: ✅ **v0.21.2 confirmed correct**. Stage 12 adds no new compiler features
(only docs + audit + tests + plan-13.1 reframe); per semver, patch bump from `v0.21.0`
(Stage 12.1 baseline) to `v0.21.2` (Stage 12.2 + 12.3 + future 12.4-12.8) is appropriate.
`v0.22.0` is reserved for **Stage 13 P0 closure** when actual compiler features ship:
- TD-030 closure call lowering (200-400 LOC, ≤5 files)
- TD-031 if-let/while-let (300-500 LOC, new AST/HIR variants)
- TD-032 macro_rules! + 19 missing built-in macros (1500-2500 LOC, new `src/macro_expand/` subsystem)

**Version history correction**:
- `v0.21.0`: Stage 12.1 (v0.1 release + v0.3 bootstrap prep) — shipped
- `v0.21.1`: (skipped — no intermediate release; reserved for hypothetical patch)
- **`v0.21.2`: Stage 12.2 + 12.3 + 12.4-12.8 (cross-stage audits r216/r217 + corrections + final gate)** — target
- `v0.22.0`: Stage 13 P0 closure (TD-030 + TD-031 + TD-032) — future, contingent on Stage 12 close

**Action for Stage 12.6**:
1. `Cargo.toml` line 3: `version = "0.22.0"` → `version = "0.21.2"`
2. `Cargo.toml` line 6 `description` field: append `+ Stage 8.1-8.7 v0.2 features + Stage 9.1-9.12 conformance expansion + Stage 10.0-10.8 CLI/runner upgrade + Stage 11.1-11.10 v0.1 gate reached + Stage 12.1-12.8 cross-stage audit r216/r217` (currently truncated at Stage 7.8 per r217-stages-5-8 §6.3)
3. `README.md:11`: `v0.22.0` → `v0.21.2`
4. `README.md:25`: `Stage 12: ✅ Complete` → `Stage 12: 🔄 In Progress (12.1-12.3 done; 12.4-12.8 pending)`
5. `README.md:26`: `Stage 13: 🔄 Planned` → `Stage 13: 📋 Draft (Stage 12 output, awaiting Stage 12 close)`
6. `README.md:14`: `Cross-stage audit r216 complete` → `Cross-stage audits r216 + r217 second-pass complete`
7. `README.md:31`: `(ratified by r216 audit)` → `(ratified by r216 + r217 second-pass audits)`
8. `RELEASE_NOTES.md`: add `v0.21.2` entry documenting Stage 12.2 + 12.3 audit work
9. `api-naming-standard.md`: update Stage 12 section to reflect `v0.21.2` (currently says `v0.22.0`)

---

## 6. Cross-stage 0-11 Pattern Analysis (FINAL synthesis)

This section synthesizes findings across all 12 prior stages (0-11) to identify
cross-stage patterns. It draws on:
- r216-architecture (D1+D5, 350 lines)
- r216-techdebt-tests-docs (D2+D3+D4+D6+D7, 650 lines)
- r217-stages-0-4 (411 lines)
- r217-stages-5-8 (~360 lines)
- This report (Stages 9-11 + Stage 12 scope)

### 6.1 TD closure trajectory

Tech-debt items introduced and closed per stage, tracked via `api-naming-standard.md` +
`stage-0-4-cross-stage-deep-review-r49.md` + r216-techdebt-tests-docs:

| Stage(s) | TDs opened | TDs closed | Net change | Notes |
|----------|-----------|------------|------------|-------|
| **Stage 0-4** (combined) | TD-001..TD-016 (16 items) | 0 (all deferred to Stage 5+) | +16 | Foundation pipeline TDs (HirParam duplication, Emitter trait breadth, AST naming, visibility, prelude, NLL single-pass, use paths, closures inline, macro_rules!, region placeholder, L-COPY-ADT, trait dispatch); all P2/P3, all deferred per "Stage 5+ will close" rationale |
| **Stage 5** (99 sub-stages) | TD-014-rep (trait dispatch vtable), TD-016-rep (dyn Trait return I32 placeholder, Stage 5.79), TD-017 (codegen/mod.rs 2398 LOC, 5.81), TD-018 (dyn Trait stdlib-only, 5.90) — **4 new + 16 carried** | TD-014 (5.80), TD-016-rep (5.82) — **2 closed** | +2 net | Trait dispatch + vtable + dyn Trait MIR infrastructure; introduced 3 new architectural TDs (file LOC + design gap + missing feature); closed 2 (trait dispatch vtable + return type refinement) |
| **Stage 6** (18 sub-stages) | TD-011 (mir/lower/mod.rs 3346 LOC, deferred from Stage 5 era), TD-019 (expr_operand giant match, 6.10 extraction byproduct), TD-022 (parser.rs 3112 LOC, 6.12), TD-023 (lexer/reader.rs, 6.13), TD-024 (borrowck/mod.rs 1452 LOC, 6.14), TD-025 (typeck/checker.rs 1320 LOC, 6.15), TD-026 (resolve/resolver.rs 1131 LOC, 6.16), TD-027 (expr_operand sub-split, 6.17 — reverted) — **8 new + 4 carried** | TD-011 (6.10, 7-step split: mod.rs 3346→772 LOC −76.9%), TD-017 (6.8), TD-022 (6.12), TD-023 (6.13), TD-024 (6.14), TD-025 (6.15), TD-026 (6.16), TD-027 (6.18, reverted) — **8 closed** | 0 net | Pure architectural refactoring stage; introduced+closed 6 file-LOC TDs in same stage; TD-019 remains on user hold; TD-027 introduced+reverted (net 0) |
| **Stage 7** (9 sub-stages) | 0 new — **2 carried** (TD-015 region inference, TD-018 user-defined trait dyn) | TD-015 (7.5, 5-step region inference complete: 1462 LOC `src/borrowck/region_inference.rs`), TD-018 (7.6, user-defined trait dyn support) — **2 closed** | −2 net | Region inference + user-defined trait dyn; closed 2 long-deferred TDs (TD-015 originally Stage 5+ deferral, TD-018 originally Stage 6+ deferral) |
| **Stage 8** (7 sub-stages) | 0 new — **0 carried open** (all Stage 0-7 TDs closed except TD-019 on hold) | 0 closed | 0 net | v0.2 features (lifetime elision, object safety, extern "C" ABI, drop elaboration, async/await MVP); no new TD introduced, no existing TD closed |
| **Stage 9** (12 sub-stages) | 0 new | 0 closed | 0 net | Parse conformance expansion (8 → 600 tests); pure test addition, no architectural impact |
| **Stage 10** (9 files; 8 conformance + 1 infra) | 0 new | 0 closed | 0 net | CLI upgrade + 8 conformance categories created (600 → 1139 tests); no architectural impact |
| **Stage 11** (10 sub-stages) | 0 new | 0 closed | 0 net | Per-category conformance expansion (1139 → 5026 tests, v0.1 gate reached); pure test addition |
| **Stage 12** (so far 3 of 8 sub-stages) | TD-028 (§16 violation mir::dyn_trait → codegen), TD-029 (TyKind::Dynamic missing), TD-030 (closure call lowering), TD-031 (if-let/while-let), TD-032 (macro_rules!), TD-033 (6 P1 sub-items: for-loop, move closure, HRTB, assoc type normalization, two-phase borrows, disjoint closure captures) — **6 new** | 0 closed | +6 net | Cross-stage audits r216 (12.2) + r217 (12.3) discovered 6 previously-implicit TDs; r217 reattributed TD-029 root cause to Stage 2.1 (not Stage 5) and corrected TD-030/031/032 numeric claims |

**TD trajectory summary**:

```
Stage 0-4: +16 opened, 0 closed → 16 open (all deferred)
Stage 5:   +4 opened, 2 closed → 18 open (16 carried + 2 new)
Stage 6:   +8 opened, 8 closed → 18 open (TD-019 + TD-027 introduced+closed/reverted, TD-011 closed)
Stage 7:   +0 opened, 2 closed → 16 open (TD-015 + TD-018 closed)
Stage 8:   +0 opened, 0 closed → 16 open
Stage 9:   +0 opened, 0 closed → 16 open
Stage 10:  +0 opened, 0 closed → 16 open
Stage 11:  +0 opened, 0 closed → 16 open
Stage 12:  +6 opened, 0 closed → 22 open (7 tracked: TD-019 + TD-028..TD-033; 15 historically closed)
```

**Pattern observations**:

1. **Stages 0-4 were "TD accumulation" stages** — 16 TDs opened, 0 closed. The early-stage
   philosophy was "build the foundation, defer the polish" (per r49 deep-review: "可接受的
   （有明确偿还计划）：TD-001 到 TD-016 全部" — all 16 accepted with explicit repayment plans).
2. **Stage 5 was "TD partial repayment"** — closed TD-014 (trait dispatch vtable, the
   core Stage 5 deliverable) and TD-016-rep (return type refinement) but introduced 3 new
   architectural TDs (file LOC from feature growth + design gap + missing feature).
3. **Stage 6 was "pure architectural TD closure"** — 8 file-LOC TDs introduced AND closed
   in the same stage (TD-022..TD-027 immediate close; TD-011 7-step split closed). Net
   change = 0. TD-019 (extraction byproduct) remains on user hold.
4. **Stage 7 was "long-deferred TD closure"** — closed TD-015 (region inference, deferred
   since Stage 5+) and TD-018 (user-defined trait dyn, deferred since Stage 5.90). Both
   were Stage 5-era deferrals that Stage 7 finally executed.
5. **Stages 8-11 were "feature/conformance stages with no new architectural TD"** — Stage
   8 added 5 v0.2 features without introducing new TDs; Stages 9-11 added 5018 conformance
   tests without introducing new TDs. This reflects the architectural foundation being
   stable enough to absorb feature/test work without debt accumulation.
6. **Stage 12 is "audit discovery stage"** — 6 new TDs (TD-028..TD-033) discovered by the
   r216 cross-stage audit, all pre-existing conditions that had been implicit. r217
   second-pass corrected 5 numeric/framing errors in the r216 attributions but did not
   add or remove TDs. Stage 12 closes 0 TDs (all 6 new TDs scheduled for Stage 13 closure).

**Net open TD count at end of Stage 12.3**: **7** (TD-019 user-hold + TD-028..TD-033).
**Of those, 3 are P0** (TD-030 closure call, TD-031 if-let/while-let, TD-032 macro_rules!)
**+ 1 is P1** (TD-033 with 6 sub-items) **+ 2 are P2** (TD-028 §16 violation, TD-029
TyKind::Dynamic) **+ 1 is P3** (TD-019 on user hold).

### 6.2 §25.8 write-back coverage

§25.8 (design deviation write-back) was introduced in `stage-committee-process.md v3.21`
at Stage 6.11. Per-stage coverage:

| Stage | §25.8 applied? | Where | Notes |
|-------|----------------|-------|-------|
| Stage 0 | ❌ NO | — | Process v3.20 or earlier; Stage 0 deep-reviews lack B1-B4 deviation analysis |
| Stage 1 | ❌ NO | — | Process v3.20 |
| Stage 2 | ❌ NO | — | Process v3.20 |
| Stage 3 | ❌ NO | — | Process v3.20 |
| Stage 4 | ❌ NO | — | Process v3.20 |
| **Stage 5** | ❌ **NO** | — | Process v3.20 (Stage 5 ran on v3.20 per `dev-log.md:242`); 7 deep-reviews (#1-#7) lack B1-B4 analysis; **never received retroactive backfill** — this is the Stage 12.4 scope |
| **Stage 6** | ✅ YES | 6.11 (introduction) + **6.18** (stage-finale full write-back) | §25.8 introduced at 6.11; full design-doc sync at 6.18 covering 7 architectural TDs |
| **Stage 7** | ✅ YES | **7.7** (TD-015 + TD-018 write-back to `03-type-system.md` +§11 + `04-ownership-borrowing.md` +§12) | Stage-finale discipline established |
| **Stage 8** | ✅ YES | **8.6** (5 v0.2 features write-back to 4 design docs: `03-type-system.md` +§12, `04-ownership-borrowing.md` +§13, `05-ast.md` +§14, `07-codegen.md` +§15) | Stage-finale discipline maintained |
| **Stage 9** | ✅ YES | **9.12** (deep-review-stage9-r195.md §2.5 D5 explicitly cites §25.8; 9 core design docs marked synced; 0 new deviations) | Stage-finale applied as design-doc sync verification (conformance stages don't introduce new design) |
| **Stage 10** | ✅ YES | **10.8** (deep-review-stage10-r205.md §5.5 line 41: "19 份设计文档全部已通过 §25.8 同步") | Stage-finale applied as design-doc sync verification |
| **Stage 11** | ⚠️ PARTIAL | **11.10** (gate-review-11.10.md §25 deep review table cites "D5 Design | ✅ 19 docs synced, conformance as executable spec" but **no separate `deep-review-stage11-rXXX.md` file exists**) | Process-compliance variance: deep-review folded into gate-review rather than separate file |
| **Stage 12** | ✅ YES | **12.2** (r216 architecture audit discovered TD-029 + wrote back to `03-type-system.md` §13) + **12.3** (this r217 second-pass audit) | Audit stages apply §25.8 as discovery + write-back of previously-implicit deviations |

**Coverage summary**:
- ✅ Stages with full §25.8: 6, 7, 8, 9, 10, 12 (6 stages)
- ⚠️ Stages with partial §25.8: 11 (deep-review folded into gate-review; no separate file)
- ❌ Stages with no §25.8: 0, 1, 2, 3, 4, 5 (6 stages — pre-v3.21 process)

**Coverage gap**: Stage 5 is the **only** stage that (a) ran on pre-v3.21 process AND
(b) has **never received retroactive §25.8 backfill**. Stages 0-4 received retroactive
backfill at Stage 6.18 (per r217-stages-0-4 §3 finding: "all retroactive at Stage 6.18").
Stage 5's 3 implicit-knowledge items (DynTraitMIRSummary, StdlibTypeKind converter,
stdlib semantic grouping) were identified in r217-stages-5-8 §2.4 and are scheduled for
Stage 12.4 closure.

**Pattern**: §25.8 discipline was **introduced** at Stage 6.11, **established as
stage-finale** at Stage 6.18, and **maintained** through Stages 7-10. Stage 11 broke the
"separate deep-review file" pattern (folded into gate-review). Stage 12 restored the
pattern via dedicated audit reports. Stage 5 is the lone pre-v3.21 stage without retroactive
backfill — Stage 12.4 closes this gap.

### 6.3 Design-doc-vs-implementation delta trend

The design-doc-vs-implementation delta is tracked via B1 (impl < design), B2 (impl >
design), B3 (impl ≠ design, accepted), B4 (design gray area, written back) deviation
categories per the §25.8 protocol.

**v0.1 baseline (end of Stage 11, per r216-architecture §3.1-§3.5)**:

| Deviation type | Count | Examples | Scheduled closure |
|----------------|-------|----------|-------------------|
| **B1 (impl < design)** | **18** | Closure call lowering (TD-030), if-let/while-let (TD-031), macro_rules! (TD-032), for-loop, move closure, HRTB, assoc type normalization, two-phase borrows, disjoint closure captures (TD-033 6 sub-items), `?Sized`, `Send`/`Sync`, `impl Trait` return, `extern "Rust"`, `#[may_dangle]`, negative impls, let-else, box patterns, monomorphization collection, name mangling, TyKind::Dynamic (TD-029), §16 violation (TD-028) | 9 scheduled for Stage 13 (3 P0 + 6 P1 sub-items); 9 long-term v0.2+ |
| **B2 (impl > design)** | **0** | — | — |
| **B3 (impl ≠ design, accepted)** | **7** | L1 PHI optimization rejected (mem2reg), Terminator::Unreachable default, simplified subtyping, simplified maybe-init, simplified NLL 3-phase, scope-order drop, basic borrow diagnostics | All accepted; no action |
| **B4 (design gray area, written back)** | **3** | MirBody.dyn_trait_calls side-table, vtable layout, mir::dyn_trait emit_* functions (TD-028) | All written back to design docs |

**r217 second-pass impact on B1 count**:

| TD | r216 attribution | r217 revision | Net B1 count change |
|----|------------------|---------------|---------------------|
| TD-028 (§16 violation) | "NEW finding" | ✅ r216 attribution CORRECT (7 emit_* functions, ≤3 files fix); root-cause refined to Stage 3.4 Emitter trait | 0 (already counted in B1) |
| TD-029 (TyKind::Dynamic) | "NEW finding" (B1) | ⚠️ NEW at MIR level only; AST/HIR implement TraitObject; reattributed to Stage 2.1; §25.8 write-back scope expanded to include `06-mir.md` §14 | 0 (still a B1 deviation, just better-attributed) |
| TD-030 (closure call) | "41 FAIL tests in 3 closure dirs" | ❌ r216 numeric WRONG: 0 `//! FAIL` markers in cited dirs; 40 `compile_error` tests across whole conformance; methodology error conflating FAIL with compile_error | 0 (still a B1 deviation; FAIL count corrected from 41 to 0, but conformance still has 40 compile_error tests exercising closure error paths) |
| TD-031 (if-let/while-let) | "12 FAIL in 00-parse/02-control-flow/" (techdebt) vs "6+5=11 in 02-borrowck/01-nll-advanced/" (architecture) — internal inconsistency | ❌ Both wrong: 11 actual `//! FAIL` tests (6 if-let + 5 while-let) in `00-parse/02-control-flow/`; reattributed to Stage 0 parser scope | 0 (still a B1 deviation; FAIL count corrected from 12 to 11, location corrected from 02-borrowck to 00-parse) |
| TD-032 (macro_rules!) | "26 built-in macros hardcoded" | ❌ r216 framing INVERTED: 7 of 26 hardcoded; 19 missing (not 26 hardcoded) | 0 (still a B1 deviation; hardcode count corrected from 26 to 7, missing count corrected from 0 to 19) |

**Net B1 count change from r217**: **0** (r217 corrected attributions and numerics but
did not add or remove B1 deviations). The 18 B1 count from r216 stands.

**Trend analysis**:

| Snapshot | B1 count | B2 | B3 | B4 | Notes |
|----------|---------|----|----|----|----|
| End Stage 11 (r216 baseline) | 18 | 0 | 7 | 3 | First comprehensive §25.8 audit; 18 B1 gaps identified |
| End Stage 12.3 (r217 second-pass) | 18 | 0 | 7 | 3 | Numeric/framing corrections only; no B1 add/remove |
| Stage 13 target (P0 + P1 closure) | 9 | 0 | 7 | 3 | 3 P0 (TD-030/031/032) + 6 P1 sub-items (TD-033) = 9 B1 closures scheduled |
| v0.2+ target (long-term) | 0 | 0 | 7 | 3 | Remaining 9 B1 items are v0.2+ features (`?Sized`, `Send`/`Sync`, `impl Trait` return, `extern "Rust"`, `#[may_dangle]`, negative impls, let-else, box patterns, monomorphization, name mangling, TD-028 §16 fix, TD-029 TyKind::Dynamic impl) |

**Verdict**: The design-doc-vs-implementation delta is **closing slowly**. The v0.1
baseline of 18 B1 deviations will drop to 9 after Stage 13 P0+P1 closure (50% reduction),
then to 0 over v0.2-v0.4 as long-term features ship. The **rate of closure** is gated by:

1. **Stage 13 P0 blockers** (3 items) — required for v0.3 self-hosting; estimated 4-8 weeks
2. **Stage 13 P1 sub-items** (6 items) — required for Stage 1 source drafting; concurrent with Stage 13.5+
3. **v0.2+ features** (9 items) — long-term; not blocking self-hosting

The gap is **not widening** — Stages 8-12 introduced 0 new B1 deviations (only discovered
existing ones via audit). The gap is **closing at the planned rate** (Stage 13 will close
50% of the v0.1 baseline B1 count).

---

## 7. Recommendations for Stage 12.4+ Execution

Prioritized action list with owner + estimated effort. Owners refer to the agent roles
defined in `stage-committee-process.md` §1.4 (PM-A = project manager, REC-A = records
archivist, ARCH-A = architect, DEV-A = developer, QA-A = quality assurance).

### P0 — Must close in Stage 12.4-12.5 (blocks Stage 12.6+)

| # | Action | Owner | Effort | Sub-stage |
|---|--------|-------|--------|-----------|
| 1 | **§25.8 Stage 5 retroactive backfill** — add `DynTraitMIRSummary` to `06-mir.md` §6 (between DynTraitMethodCall and DynTraitMIRPlan in the 4-layer MIR infrastructure description); add `StdlibTypeKind` + `stdlib_type_kind_to_emit_type()` to `03-type-system.md` §2 or `09-stdlib.md` (TD-016 closure converter); verify stdlib semantic grouping (5 categories, 43 traits) is captured (already in `09-stdlib.md:1018` ✓) | REC-A + ARCH-A | 4-6 hours (3 design-doc edits, 5-20 lines each) | **12.4** |
| 2 | **Reframe `plan-13.1.md`** as Stage 12 output — update header from `🔄 Planned` to `📋 Draft (Stage 12 output, awaiting Stage 12 close)`; add top-of-file banner; cross-reference from `stage-12/README.md`; update `Task ID` references from `stage13.1-muv*-r217` to `stage12.5-plan13-draft-r217` | REC-A + PM-A | 30 minutes (header edit + banner + cross-reference) | **12.5** |

### P0 — Must close in Stage 12.6 (blocks Stage 12.7+)

| # | Action | Owner | Effort | Sub-stage |
|---|--------|-------|--------|-----------|
| 3 | **Revert Cargo.toml version** `0.22.0` → `0.21.2` | REC-A | 5 minutes (1-line edit) | **12.6** |
| 4 | **Sync README.md** — line 11 (`v0.22.0` → `v0.21.2`); line 14 (add r217 second-pass mention); line 25 (`Stage 12: ✅ Complete` → `Stage 12: 🔄 In Progress`); line 26 (`Stage 13: 🔄 Planned` → `Stage 13: 📋 Draft`); line 31 (add r217 mention) | REC-A + PM-A | 15 minutes (5 line edits) | **12.6** |
| 5 | **Update RELEASE_NOTES.md** — add `v0.21.2` entry documenting Stage 12.2 + 12.3 audit work + 12.4-12.8 close plan | REC-A | 20 minutes (new release-notes section) | **12.6** |
| 6 | **Update api-naming-standard.md** Stage 12 section — change version references from `v0.22.0` to `v0.21.2`; add r217 second-pass audit references | REC-A + ARCH-A | 30 minutes (section update) | **12.6** |
| 7 | **Update Cargo.toml description field** — append `+ Stage 8.1-8.7 v0.2 features + Stage 9.1-9.12 conformance expansion + Stage 10.0-10.8 CLI/runner upgrade + Stage 11.1-11.10 v0.1 gate reached + Stage 12.1-12.8 cross-stage audit r216/r217` (currently truncated at Stage 7.8 per r217-stages-5-8 §6.3) | REC-A | 10 minutes (description field expansion) | **12.6** |

### P0 — Must close in Stage 12.7 (blocks Stage 12.8)

| # | Action | Owner | Effort | Sub-stage |
|---|--------|-------|--------|-----------|
| 8 | **Correct Stage 0 README** — fix `ast_structure` test count off-by-1 (README says 149, actual is 150); remove phantom "1 misc" entry | REC-A + QA-A | 15 minutes | **12.7** |
| 9 | **Correct Stage 1 README** — fix 3 of 4 module test counts (hir_lowering 30→36, hir_resolution 25→26, hir_scope_resolution 24→17); total 99 unchanged | REC-A + QA-A | 20 minutes | **12.7** |
| 10 | **Correct Stage 2 README** — fix all 4 module test counts (integration 35→58, mir_lowering 45→22, negative_cases 30→35, typeck_borrowck 31→26); total 141 unchanged; rename `typeck_borrowck` to `typeck` (actual filename) | REC-A + QA-A | 25 minutes | **12.7** |
| 11 | **Correct Stage 3 README** — add missing `deep_inspection_tests.rs` entry (15 tests); total 309 unchanged | REC-A + QA-A | 10 minutes | **12.7** |
| 12 | **Correct Stage 4 README** — replace nonexistent `module_tests.rs` with `visibility_tests.rs`; add missing `closure_full_call_tests.rs` entry (2 tests); fix `closure_call` count (4→2), `closure_capture` count (3→4), `macro` count (2→3, rename to `macro_system`); add `visibility` entry (2 tests); total 13 unchanged | REC-A + QA-A | 30 minutes | **12.7** |

### P0 — Must close in Stage 12.8 (closes Stage 12)

| # | Action | Owner | Effort | Sub-stage |
|---|--------|-------|--------|-----------|
| 13 | **Produce `gate-review-12.8.md`** — §25 deep review of Stage 12 itself; D1-D7 dimensions evaluated; 5/5 GO → PASS vote; explicit Stage 13 launch authorization line (if all 5 launch criteria met); cross-reference to r216 + r217 audits | PM-A + ARCH-A + REC-A + QA-A (full committee) | 2-4 hours | **12.8** |
| 14 | **Produce `deep-review-stage12-rXXX.md`** — dedicated deep-review file (unlike Stage 11 which folded deep-review into gate-review); restore the pattern from Stages 6-10 | ARCH-A + QA-A | 2-4 hours | **12.8** |
| 15 | **Update Stage 12 README** — add 12.3-12.8 sub-stage entries; mark Stage 12 complete; cross-reference Stage 13 launch criteria | REC-A | 30 minutes | **12.8** |

### P1 — Should close in Stage 12.4-12.7 (parallel with P0)

| # | Action | Owner | Effort | Sub-stage |
|---|--------|-------|--------|-----------|
| 16 | **Stage 6 plan discipline backfill** — create retroactive `plan-6.4.md`, `plan-6.5.md`, `plan-6.6.md` from existing gate reviews (TD-011 step 4-6 ran without separate plans; per r217-stages-5-8 §3.1) | REC-A | 1-2 hours | **12.4** (parallel) |
| 17 | **Stage 5 README.md creation** — Stage 5 is the largest stage (99 sub-stages, 977 tests, 502 conformance) but has NO `README.md`; r216-techdebt flagged this, r217-stages-5-8 §7 re-confirmed; mirror structure of `stage-6/README.md` | REC-A | 2-3 hours | **12.4** (parallel) |
| 18 | **Stage 8 §25.8 backfill** — add async/await "MVP synchronous" lowering decision to `05-ast.md` +§14 or `07-codegen.md` +§15 (currently only in `src/ast/async_marker.rs:1-14` module comment; per r217-stages-5-8 §5.2) | ARCH-A + REC-A | 30 minutes | **12.4** (parallel) |
| 19 | **Stage 10.8 gate-review errata** — fix internally inconsistent "8/8 sub-stages (10.0-10.8)" wording (9 files listed, 8 counted; see §3.1 of this report); recommend "8/8 conformance sub-stages (10.1-10.8) + 1 infra-prep stage (10.0)" | REC-A | 10 minutes | **12.6** (parallel) |
| 20 | **Stage 11.10 deep-review file creation** — extract §25 deep review from `gate-review-11.10.md` into separate `deep-review-stage11-rXXX.md` to restore the Stages 6-10 pattern (per §4.1 of this report) | REC-A | 1 hour | **12.7** (parallel) |

### P2 — Should close before Stage 13.1 implementation (post-Stage 12)

| # | Action | Owner | Effort | When |
|---|--------|-------|--------|------|
| 21 | **Apply TD-030/031/032 numeric corrections to r216 docs** — or supersede r216 with r217 as the authoritative baseline (per r217-stages-0-4 §7 P0) | ARCH-A + REC-A | 1-2 hours | Stage 12.4 (parallel) |
| 22 | **Expand TD-029 §25.8 write-back scope** to include `06-mir.md` §14 (currently only `03-type-system.md` §10/§11/§12 per r216; r217-stages-0-4 §2.3 expanded scope) | ARCH-A | 1 hour | Stage 12.4 (parallel) |
| 23 | **Add Stage 3.4 Emitter trait root-cause note** to TD-028 §25.8 write-back in `06-mir.md` §14 (per r217-stages-0-4 §2.2) | ARCH-A | 30 minutes | Stage 12.4 (parallel) |
| 24 | **Update `src/mir/lower/expr_operand.rs:876` code comment** to reflect r217 verification of TD-030 (per r217-stages-0-4 §7 P2) | ARCH-A | 15 minutes | Stage 12.4 (parallel) |
| 25 | **Test-to-source-LOC ratio monitoring** — add to Stage 13 risk register: dedicated test:src ratio peaked at 0.085 (Stage 5 end), declined to 0.069 (Stage 12 end); set floor of 0.075 for v0.3 (per r217-stages-5-8 §7 P2) | QA-A | 30 minutes | Stage 12.8 (parallel) |

### P3 — Optional, can defer to Stage 13

| # | Action | Owner | Effort | When |
|---|--------|-------|--------|------|
| 26 | **Stage 0-4 implicit-knowledge backfill** — 15 items identified in r217-stages-0-4 §3 (3 per stage × 5 stages); each is 5-20 lines in target design doc | ARCH-A + REC-A | 4-6 hours | Stage 12.4-12.7 (parallel, can defer to Stage 13) |
| 27 | **Stage 5 deep-review #1-#7 §25.8 retrofit** — retrofit B1-B4 deviation analysis onto Stage 5's 7 deep-reviews (which ran on v3.20 process, pre-§25.8); informational only | ARCH-A | 2-3 hours | Stage 13 (optional) |
| 28 | **TD-018/TD-028 scope overlap note in Stage 13.1 plan** — both target `src/mir/dyn_trait.rs` (954 LOC); Stage 13.1 TD-028 fix should preserve TD-018 closure state (per r217-stages-5-8 §2.6) | ARCH-A | 15 minutes | Stage 12.5 (parallel, can defer) |

**Total estimated effort for Stage 12.4-12.8**: **~20-30 hours** of agent work (P0 items
1-15: ~12-18 hours; P1 items 16-20: ~5-7 hours; P2 items 21-25: ~4-5 hours parallel;
P3 items 26-28: ~6-9 hours optional).

---

## 8. Committee Vote (combined PM-A + REC-A + ARCH-A)

### Vote: **GO-WITH-CONDITIONS**

### Rationale

**Strengths confirmed** (Stages 9-11 + Stage 12 scope):

- ✅ Stage 9: 12 sub-stages + 600 parse conformance tests + 145 rust tests + 12 docs/tests
  .md files + §25.8 applied at 9.12 — **every numeric claim verified exactly**.
- ✅ Stage 10: 9 gate-review files (10.0-10.8, 1 infra + 8 conformance) + 1139 conformance
  at end + all 4 CLI flags present + 8 conformance categories created + `--mode auto` runner
  default — **every numeric claim verified exactly** (with one internal inconsistency
  noted in `gate-review-10.8.md:36` "8/8 (10.0-10.8)" wording).
- ✅ Stage 11: 10 sub-stages + 5026 conformance total + per-category breakdown
  600/1020/800/601/502/500/502/501 + 5/5 GO → PASS at 11.10 + README v0.1 gate messaging
  — **every numeric claim verified exactly**.
- ✅ Stage 12 scope finalized: 8 sub-stages (12.1-12.8) with clear DONE/CURRENT/PENDING
  status; 5 Stage 13 launch criteria defined; version policy confirmed as v0.21.2 (patch
  bump, not v0.22.0 minor).
- ✅ TD closure trajectory documented: 16 opened in Stages 0-4, 7 closed in Stages 5-7
  (TD-014/016/011/017/015/018/022-027), 6 new discovered in Stage 12 (TD-028..TD-033);
  net open = 7 (3 P0 + 1 P1 + 2 P2 + 1 P3 on hold).
- ✅ §25.8 coverage analysis: 6 stages with full §25.8 (6, 7, 8, 9, 10, 12), 1 stage
  partial (11 — folded into gate-review), 6 stages without (0-5, pre-v3.21 process);
  Stage 5 is the lone pre-v3.21 stage without retroactive backfill — Stage 12.4 closes
  this gap.
- ✅ Design-doc-vs-implementation delta trend documented: 18 B1 deviations at v0.1
  baseline; 9 scheduled for Stage 13 closure (50% reduction); 9 long-term v0.2+; gap is
  closing at planned rate, not widening.

**Conditions for GO** (must close in Stage 12.4-12.8 before Stage 13 launch):

1. **Stage 12.4 §25.8 Stage 5 backfill** — 3 design-doc edits adding DynTraitMIRSummary,
   StdlibTypeKind converter, and (verify) stdlib semantic grouping.
2. **Stage 12.5 `plan-13.1.md` reframe** — header change + banner + cross-reference;
   change `🔄 Planned` to `📋 Draft (Stage 12 output, awaiting Stage 12 close)`.
3. **Stage 12.6 version revert + sync** — Cargo.toml `0.22.0` → `0.21.2`; README.md 5
   line edits; RELEASE_NOTES.md new entry; api-naming-standard.md section update; Cargo.toml
   description field expansion.
4. **Stage 12.7 Stage 0-4 README corrections** — 5 README files with per-module test count
   corrections (totals are correct); fix nonexistent `module_tests.rs` reference.
5. **Stage 12.8 final gate review** — `gate-review-12.8.md` + `deep-review-stage12-rXXX.md`
   with 5/5 GO → PASS; explicit Stage 13 launch authorization (if all 5 launch criteria met).

**Stage 13 launch authorization**: **NOT GRANTED**. Stage 12.3 (this r217 second-pass
audit) is complete; Stage 12.4-12.8 must close the 5 conditions above before Stage 13.1
implementation begins. This is consistent with r217-stages-0-4 §6 and r217-stages-5-8 §8
recommendations.

**For v0.1 release**: ✅ **STILL RATIFIED** — r217 does not affect v0.1 release artifact
(5026/5000 conformance gate holds; the r216 numeric errors corrected by r217 are in audit
documentation, not in the conformance test suite itself or in the compiler code).

**For v0.3 self-hosting target**: ⚠️ **CONTINGENT** on Stage 13 executing the corrected
TD-030, TD-031, TD-032 closure plan (with r217-verified numbers) — same conditional as
r216, but with corrected baseline data.

**Final note**: Stages 9-11 are **architecturally clean** conformance-driven stages that
introduced **zero new tech debt** and closed **zero existing tech debt** — they are pure
verification + test-addition stages. Their contribution to the project is the **5026-test
executable spec** that ratifies the v0.1 gate and provides the contract for Stage 13 P0
closure work. Stage 12 is the **audit + governance stage** that discovered 6 previously-
implicit TDs (TD-028..TD-033), corrected 5 numeric/framing errors via second-pass review,
and now must close 5 documentation/discipline conditions before Stage 13 can launch. The
codebase is **ready for v0.3 self-hosting preparation** once Stage 12.4-12.8 closes the
documentation gaps identified across r217-stages-0-4 + r217-stages-5-8 + this report.

---

**Audit completed**: 2026-07-26

**Companion audits**:
- `cross-stage-audit-r216-architecture.md` (ARCH-A, D1+D5, GO-WITH-CONDITIONS) — to be
  updated with r217 corrections in Stage 12.4
- `cross-stage-audit-r216-techdebt-tests-docs.md` (combined, D2+D3+D4+D6+D7,
  GO-WITH-CONDITIONS) — to be updated with r217 corrections in Stage 12.4
- `cross-stage-audit-r217-stages-0-4.md` (ARCH-A + REV-A, second-pass for Stages 0-4,
  GO-WITH-CONDITIONS) — completed
- `cross-stage-audit-r217-stages-5-8.md` (ARCH-A + REV-A + QA-A, second-pass for Stages
  5-8, GO-WITH-CONDITIONS) — completed
- This report: `cross-stage-audit-r217-stages-9-12-scope.md` (PM-A + REC-A + ARCH-A,
  second-pass for Stages 9-11 + Stage 12 scope finalization, GO-WITH-CONDITIONS) — completed

**Next action**: Stage 12.4 — §25.8 Stage 5 retroactive backfill (3 design-doc edits) +
parallel: Stage 6 plan-6.{4,5,6}.md retroactive backfill + Stage 5 README.md creation +
Stage 8 §25.8 async/await backfill.
