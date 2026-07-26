# Second-Pass Cross-Stage Audit (r217) — Stage Round Revision + Stages 0-4 Re-audit

> **Auditor**: ARCH-A + REV-A (combined for second-pass)
> **Date**: 2026-07-26
> **Baseline**: v0.21.0 → v0.21.2 (target patch bump; see §4.3 for rationale)
> **Scope**: r216 revision + stages 0-4 systematic re-audit + Stage 12/13 framing correction
> **Companion**:
> - `cross-stage-audit-r216-architecture.md` (350 lines, ARCH-A, D1+D5)
> - `cross-stage-audit-r216-techdebt-tests-docs.md` (650 lines, combined, D2+D3+D4+D6+D7)

---

## 1. Executive Summary

The second-pass audit (r217) re-verifies every numeric claim in the r216 baseline against
actual file contents and grep results. **Three of five TD items (TD-030, TD-031, TD-032)
carry numerical errors** that must be corrected before Stage 13 work begins; the other two
(TD-028, TD-029) require **framing refinements** rather than numeric corrections.
Stages 0-4 systematic re-audit confirms that all 5 test-plan/README.md files (created in
Stage 12.2 D7 backfill) carry **correct total test counts** but **incorrect per-module
breakdowns** — the totals match `cargo test --test all_tests` but the README tables
misattribute tests to wrong modules. A critical **implicit-knowledge gap** is uncovered:
4 of 6 design docs (`02-grammar`, `03-type-system`, `04-ownership-borrowing`, `05-ast`)
have **zero references to Stage 0-4 implementation work** in their main body — all
§25.8 write-backs were applied retroactively at Stage 6.18, leaving the original Stage
0-4 design decisions implicit in `dev-log.md` files only. Per §15 long-term > short-term,
the prematurely-launched Stage 13 plan should be **reframed as Stage 12 output** ("future-stage
planning" produced by Stage 12, executed by Stage 13), and the version bump should be
**reverted from v0.22.0 to v0.21.2** (patch, not minor) since Stage 12.2/12.3 added no
compiler features.

**Headline numbers**:
- **Stage-round revisions**: 5 (TD-028 framing refine, TD-029 framing refine + root-cause reattribute to Stage 2.1, TD-030 numeric correction, TD-031 numeric correction, TD-032 framing inversion)
- **New findings vs r216**: 8 (3 numeric TD corrections + 4 stage README per-module attribution errors + 1 design-doc implicit-knowledge gap)
- **Implicit-knowledge items identified** (Stages 0-4 combined): 15 (3 per stage)
- **Design doc §25.8 coverage for Stages 0-4**: ⚠️ partial — all retroactive at Stage 6.18/8.6, zero contemporaneous write-backs
- **Committee vote**: **GO-WITH-CONDITIONS** (Stage 12.3 corrections must close 5 TD numeric/framing errors + Cargo.toml version revert before Stage 13 launch)

---

## 2. Stage Round Revision (r216 → r217)

### 2.1 Verification methodology

For each TD item (TD-028 through TD-032), the r216 attribution was re-verified by:
1. Grep / read the actual source file cited
2. Count actual occurrences using `grep -c` / `wc -l`
3. Cross-reference against the cited conformance directories
4. Check prior deep-reviews (`docs/develop/v0/stage-{5,6,7,8,9,10,11}/deep-review-*.md`)
   for prior mentions

### 2.2 TD-028 — `mir::dyn_trait` → `codegen` §16 violation

| Field | r216 attribution | r217 verified | Revision needed? |
|-------|------------------|---------------|-------------------|
| Source location | `src/mir/dyn_trait.rs:160` (`emit_dyn_trait_fat_ptr_text` calls `crate::codegen::emit_dynptr_global_text`) | ✅ Confirmed at line 159 (`pub fn emit_dyn_trait_fat_ptr_text`) | No |
| Number of `emit_*` functions | 7 | ✅ Confirmed exactly 7 (`pub fn emit_*` matches at lines 159, 187, 211, 375, 549, 573, 767) | No |
| Stage attribution | "Stage 5.61-5.80 sub-sections" | ✅ Confirmed all 7 fall in range 5.63-5.74 (well within 5.61-5.80): emit_dyn_trait_fat_ptr_text=5.63, emit_dyn_trait_fat_ptrs_text_batch=5.64, emit_dyn_trait_fat_ptrs_text_batch_from_resolver=5.65, emit_dyn_trait_method_call_text=5.67, emit_dyn_trait_method_calls_text_batch=5.69, emit_dyn_trait_method_calls_text_batch_from_resolver=5.70, emit_dyn_trait_mir_plan_text=5.74 | No |
| Fix scope | ≤3 files (`mir/dyn_trait.rs`, `mir/mod.rs`, `codegen/trait_dispatch.rs`) | ✅ Confirmed — only these 3 files reference `emit_dyn_trait_fat_ptr_text` | No |

**Verdict**: ✅ r216 attribution CORRECT — no revision needed.

**Root-cause refinement (not in r216)**: While the §16 violation was *added* during Stage 5.63-5.74, the *architectural pattern* (MIR module producing codegen output) was enabled by Stage 3 codegen architecture (Stage 3.4 Emitter trait + Stage 3.27-3.49 typed codegen). The violation traces to Stage 5 reusing the Stage 3 Emitter pattern in a MIR module — a Stage 3 design choice (Emitter trait is `pub` and callable from any module) that became a §16 violation when Stage 5 reused it across the MIR→codegen boundary. The §25.8 write-back should note this Stage 3.4 origin, not just the Stage 5.63 introduction.

### 2.3 TD-029 — `TyKind::Dynamic` / `TraitObject` missing

| Field | r216 attribution | r217 verified | Revision needed? |
|-------|------------------|---------------|-------------------|
| Source location | `src/mir/ty.rs:28` `TyKind` enum has no `Dynamic` / `TraitObject` variant | ✅ Confirmed — `TyKind` (lines 28-62) has 17 variants (Bool through Error); no `Dynamic` / `TraitObject` | No |
| Characterization | "NEW finding" (newly-discovered B1) | ⚠️ **Framing refine**: NEW at MIR level only — `HirTy::TraitObject` exists at `src/hir/kinds.rs:536`, `Ty::TraitObject` exists at `src/ast/kinds.rs:246`, and `docs/develop/v0/stage-1/plan-1.1.md:217` explicitly listed `TraitObject` as one of 16 planned `HirTy` variants | **YES — reframe as "newly-discovered MIR-level gap"** |
| Prior deep-review mentions | "newly-uncovered" (architecture r216 §3.3) | ✅ Verified — zero matches for `Dynamic` / `TraitObject` in any `deep-review-*.md` file under stages 5-11 (13 deep-reviews checked) | No |
| Stage of origin | "r216-architecture" | ⚠️ **Reattribute**: The MIR-level gap traces to **Stage 2.1** (where `TyKind` was first defined with 16 variants, missing `Dynamic`), not Stage 5 (where dyn Trait MIR lowering was added) | **YES — reattribute to Stage 2.1 root cause** |
| §25.8 write-back target | `03-type-system.md` §10/§11/§12 | ⚠️ Should ALSO target `06-mir.md` §14 (MIR type system deviations) — currently only `03-type-system.md` is in scope | **YES — expand write-back scope** |

**Verdict**: ⚠️ Framing refinement needed. TD-029 is "newly-discovered" in the strict sense (no prior deep-review noted it) BUT the design team knew about `TraitObject` since Stage 1.1 (`plan-1.1.md:217`) and implemented it at AST + HIR levels. The gap is **MIR-level only**. The §25.8 write-back should:
1. Note that AST (`ast/kinds.rs:246`) and HIR (`hir/kinds.rs:536`) already model `TraitObject`
2. Attribute the root cause to Stage 2.1 (MIR types definition) where `Dynamic` should have been added
3. Target both `03-type-system.md` AND `06-mir.md` §14 for write-back

### 2.4 TD-030 — Closure call lowering incomplete

| Field | r216 attribution | r217 verified | Revision needed? |
|-------|------------------|---------------|-------------------|
| FAIL test count | "41 FAIL tests" | ❌ **INCORRECT**: 0 `//! FAIL` markers in the 3 cited directories (`01-typecheck/03-closures/`, `02-borrowck/03-closure-capture/`, `04-e2e/03-closures/`); 34 `EXPECTED: compile_error` tests in those dirs (11+20+3); 40 compile_error tests across all conformance mention "closure" in description | **YES — major numeric correction** |
| FAIL test location | "`01-typecheck/03-closures/`, `02-borrowck/03-closure-capture/`, `04-e2e/03-closures/`" | ⚠️ These directories contain **0 actual `//! FAIL` Stage 0 limitation markers** — they contain `// EXPECTED: compile_error` tests (working-as-intended tests demonstrating borrowck catches closure errors) | **YES — directory restriction wrong** |
| Total closure-related tests | Implied 41 | ✅ Actual closure-related `//! FAIL` markers across entire conformance tree: 1 (`00-parse/07-closures/closure_arg_basic.lin` — unrelated to call lowering); Actual `compile_error` tests with "closure" in description: 40 | **YES — recharacterize** |
| r216 methodology | "verified by grepping `^// EXPECTED: compile_error` + `^//!\s*FAIL`" | ❌ This conflates two distinct categories: `compile_error` tests are intended-failure (working tests); `//! FAIL` tests are Stage 0 limitations (broken tests). The "41" number is off-by-one from the actual 40 closure-related `compile_error` tests, AND the directory restriction is wrong. | **YES — methodology error in r216** |
| Stage of origin | Implied Stage 4.4 (closure type lowering) | ✅ Correct — Stage 4.4 added `AggregateKind::Closure` + `TyKind::Closure` but deferred call dispatch; the deferral is the root cause | No |

**Verdict**: ❌ **Numeric correction required**. The "41 FAIL tests" claim is incorrect on two axes:
1. **Count**: 0 `//! FAIL` markers (actual Stage 0 limitations) in the 3 cited directories; the 41 number traces to 40 `compile_error` tests with "closure" in description across the **entire** conformance tree, not the 3 directories.
2. **Concept**: `compile_error` tests are intended-failure tests (working as designed); `//! FAIL` tests are Stage 0 limitations (broken tests). r216 conflated these.

**Revised TD-030 statement**: "Closure call lowering incomplete — `src/mir/lower/expr_operand.rs:876` 'closure calls still go through regular Call' (deferred from Stage 4.8+). Documented as 0 `//! FAIL` Stage 0 limitations in closure test directories (closure call dispatch is functionally broken but no FAIL marker exists); 40 `compile_error` tests across conformance tree exercise closure-related error paths. Estimate: 200-400 LOC, ≤5 files."

### 2.5 TD-031 — `if let` / `while let` not in AST/HIR

| Field | r216 attribution | r217 verified | Revision needed? |
|-------|------------------|---------------|-------------------|
| FAIL test count | "12 FAIL tests in `00-parse/02-control-flow/`" (techdebt doc) vs "6 FAIL tests with 'if let' + 5 FAIL tests with 'while let' descriptions" (architecture doc, in 02-borrowck/01-nll-advanced/) | ❌ **INCORRECT + internal inconsistency**: Actual `//! FAIL` markers in `00-parse/02-control-flow/`: 15 total, of which **11 are if-let/while-let** (6 if-let + 5 while-let, by filename: `if_let_basic`, `if_let_struct`, `if_let_else`, `if_let_tuple`, `if_let_wildcard`, `if_let_chain` + `while_let_basic`, `while_let_nested`, `while_let_continue`, `while_let_break`, `while_let_tuple`); 4 others are `err_*` parse-error tests (`err_match_without_scrutinee`, `err_if_without_cond`, `err_while_without_cond`, `err_for_without_in`). Actual `//! FAIL` in `02-borrowck/01-nll-advanced/`: 0 | **YES — numeric correction** |
| r216 internal inconsistency | Architecture doc cites 6+5=11 (in 02-borrowck/01-nll-advanced/); techdebt doc cites 12 (in 00-parse/02-control-flow/) | ❌ Both wrong: 11 actual (in 00-parse/02-control-flow/, NOT 02-borrowck); r216's architecture doc got the location wrong and techdebt got the count wrong | **YES — resolve inconsistency** |
| AST/HIR gap | "Not in AST/HIR — `grep "IfLet" src/` returns zero" | ✅ Confirmed — `IfLet` / `WhileLet` variants not present in `ast/kinds.rs` or `hir/kinds.rs` | No |
| Stage of origin | Not attributed in r216 | ✅ Should be attributed to Stage 0.5 (parser item kind definitions) — the parser never grew IfLet/WhileLet arms; the omission traces to Stage 0 parser scope, not later stages | **YES — attribute to Stage 0 parser scope** |

**Verdict**: ❌ **Numeric correction required + internal inconsistency must be resolved**.
- Correct count: **11 `//! FAIL` tests** (6 if-let + 5 while-let) in `00-parse/02-control-flow/`
- Correct location: `00-parse/02-control-flow/` (not `02-borrowck/01-nll-advanced/`)

**Revised TD-031 statement**: "`if let` / `while let` not in AST/HIR — `grep "IfLet\|WhileLet" src/` returns zero matches; 11 `//! FAIL` Stage 0 limitation markers in `00-parse/02-control-flow/` (6 if-let + 5 while-let). Estimate: 300-500 LOC."

### 2.6 TD-032 — `macro_rules!` not implemented

| Field | r216 attribution | r217 verified | Revision needed? |
|-------|------------------|---------------|-------------------|
| Framing | "macro_rules! not implemented (26 built-in macros per §2.6)" | ❌ **Framing inverted**: The §2.6 spec lists **26 macros Stage 1 needs**; the **compiler hardcodes 7 of them** (not 26). The gap is **19 missing hardcoded macros**, not "26 built-in macros hardcoded". | **YES — framing inversion** |
| Actual hardcoded macros | Implied 26 hardcoded | ✅ Actual hardcoded in `src/mir/lower/expr_operand.rs:1090-1117`: **7** (`println`, `print`, `eprintln`, `eprint`, `stringify`, `assert`, `debug_assert`) — see match arms at line 1091 (`"println" | "print" | "eprintln" | "eprint"`), line 1097 (`"stringify"`), line 1112 (`"assert" | "debug_assert"`) | **YES — actual count is 7, not 26** |
| Spec target | "26 built-in macros per §2.6" | ✅ Confirmed — `13-stage1-feature-whitelist.md:135` lists exactly 26 macros | No |
| Missing macros | Not enumerated | ✅ **19 missing** (26 − 7 = 19): `format`, `write`, `writeln`, `vec`, `matches`, `assert_eq`, `assert_ne`, `debug_assert_eq`, `debug_assert_ne`, `panic`, `dbg`, `unreachable`, `todo`, `unimplemented`, `concat`, `file`, `line`, `column`, `module_path` | **YES — enumerate missing** |
| Stage of origin | Implied Stage 4.10 (MacroCall expansion) | ✅ Correct — Stage 4.10 added the MacroCall match arm with 7 hardcoded macros; the deferral to "macro_rules! subsystem" traces here | No |

**Verdict**: ❌ **Framing inversion required**. The r216 phrasing "26 built-in macros hardcoded" is incorrect — only 7 are hardcoded; 19 are missing. The correct framing per §2.6 spec:
- **Spec target**: 26 macros Stage 1 must be able to use (per `13-stage1-feature-whitelist.md` §2.6)
- **Current implementation**: 7 of 26 hardcoded in `src/mir/lower/expr_operand.rs:1090-1117`
- **Gap**: 19 missing macros (enumerated above)
- **Alternative path**: implement `macro_rules!` (full user-defined macro subsystem) instead of hardcoding all 19 missing

**Revised TD-032 statement**: "`macro_rules!` not implemented — Stage 1 contract (`13-stage1-feature-whitelist.md` §2.6) requires 26 built-in macros; current implementation hardcodes 7 of 26 in `src/mir/lower/expr_operand.rs:1090-1117` (println/print/eprintln/eprint/stringify/assert/debug_assert); 19 missing (format/write/writeln/vec/matches/assert_eq/assert_ne/debug_assert_eq/debug_assert_ne/panic/dbg/unreachable/todo/unimplemented/concat/file/line/column/module_path). Either: (a) implement `macro_rules!` subsystem + 26 built-in macro_rules! definitions, or (b) hardcode the 19 missing macros. Estimate: (a) 1500-2500 LOC new `src/macro_expand/` subsystem; (b) 400-600 LOC additional match arms in `expr_operand.rs`."

### 2.7 TD revision summary table

| TD ID | r216 attribution | r217 verified | Revision needed? | Notes |
|-------|------------------|---------------|-------------------|-------|
| TD-028 | "Stage 5.61-5.80 sub-sections", 7 emit_* functions, ≤3 files fix | ✅ All claims verified — 7 functions span 5.63-5.74, ≤3 files confirmed | No (root-cause refinement: pattern traces to Stage 3.4 Emitter trait) | r216 numeric correct |
| TD-029 | "NEW finding" — `TyKind::Dynamic` missing | ⚠️ NEW at MIR level only; AST/HIR implement `TraitObject`; gap traces to Stage 2.1 (not Stage 5) | YES — reframe as "MIR-level gap" + reattribute to Stage 2.1 + expand §25.8 write-back to `06-mir.md` §14 | r216 framing incomplete |
| TD-030 | "41 FAIL tests in 3 closure directories" | ❌ 0 `//! FAIL` markers in those dirs; 40 `compile_error` tests across whole conformance; methodology error conflating FAIL markers with compile_error tests | YES — major numeric correction + methodology fix | r216 numerically wrong |
| TD-031 | "12 FAIL tests in 00-parse/02-control-flow/" (techdebt) vs "6+5=11 in 02-borrowck/01-nll-advanced/" (architecture) — internal inconsistency | ❌ Actual: 11 `//! FAIL` tests (6 if-let + 5 while-let) in `00-parse/02-control-flow/`; r216 internal inconsistency must be resolved | YES — numeric correction + resolve internal inconsistency | r216 internally inconsistent |
| TD-032 | "26 built-in macros hardcoded" | ❌ Only 7 of 26 hardcoded; 19 missing (framing inverted) | YES — framing inversion (7 implemented, 19 missing, 26 is spec target not impl count) | r216 framing inverted |

---

## 3. Stages 0-4 Re-audit

For each stage, the audit card reports: (1) test count verification, (2) top-3 implicit-knowledge
items not in design docs, (3) §25.8 design doc coverage, (4) new findings vs r216, (5) revised
stage-round attribution.

### Stage 0 Re-audit (r217)

- **Test count verification**:
  - README claim (`docs/tests/v0/stage0/plan/README.md`): 344 total = lexer 109 + parser 85 + ast_structure 149 + 1 misc
  - Actual `#[test]` count in `tests/v0/stage0/plan/*.rs`: lexer=109, parser=85, ast_structure=**150** (off-by-1: README says 149, actual is 150); "1 misc" is unaccounted (no 4th file)
  - Total: 109 + 85 + 150 = **344** ✅ (total matches; ast_structure off-by-1 offset by phantom "1 misc")
  - Conformance 00-parse: 600 ✅ (matches README)
- **Implicit knowledge items (top 3)**:
  1. **S0-REV-1 to S0-REV-7 review rounds** — 39 P0 bugs found and fixed across 7 review rounds (16+23+7+8+5+7+4=70 individual findings); only documented in `dev-log.md` §4, not surfaced in `02-grammar.md` or `05-ast.md`. Should be in `02-grammar.md` §25.8 as "Stage 0 front-end convergence history".
  2. **P1 limitations list** (11 items: CRLF normalization, BOM error message, `RawByteStrLit` hash-count loss, `LexError` trait impls, 14 weak reserved keywords, Display `_` fallback exhaustiveness, `PathLeading::Crate/Super/Self_` variant gap, top-level `Span::DUMMY`, closure `move` keyword, `TokenKind` Eq derivation, integer overflow clamp) — only in `dev-log.md` §5.2.2. Should be in `02-grammar.md` §25.8.
  3. **Decision to skip nested items** (vs Rust) — explicitly noted in `dev-log.md` §6.2 ("不做嵌套 item（与 Rust 不同，简化）"); this is a Stage 0 design decision but is NOT in `05-ast.md` design doc. Should be written back.
- **Design doc §25.8 coverage**: ⚠️ **partial — retroactive only**. `02-grammar.md` §5 (v0.14.0 §25.8 write-back, Stage 6.18) and `05-ast.md` §13 (v0.14.0, Stage 6.18) cover Stage 0 work retroactively but were NOT written during Stage 0. Zero contemporaneous Stage 0 §25.8 write-backs exist. Main body of `02-grammar.md` has **zero** references to Stage 0.1-0.4 implementation; `05-ast.md` main body has **zero** references to Stage 0.3 AST work.
- **New findings vs r216**: ast_structure test count off-by-1 (README 149 vs actual 150).
- **Revised stage-round attribution**: TD-031 (if-let/while-let) root cause traces to **Stage 0 parser scope** — the parser never grew `IfLet`/`WhileLet` arms because they were deferred to "Stage 1+" per `dev-log.md` §5.2.3 line 228 ("struct literal / if let / while let / macro call 表达式" listed as out-of-scope). The original deferral decision is in Stage 0 dev-log.

### Stage 1 Re-audit (r217)

- **Test count verification**:
  - README claim (`docs/tests/v0/stage1/plan/README.md`): 99 total = hir_structure 20 + hir_lowering 30 + hir_resolution 25 + hir_scope_resolution 24
  - Actual `#[test]` count: hir_structure=20 ✅, hir_lowering=**36** (README says 30, off-by-6), hir_resolution=**26** (README says 25, off-by-1), hir_scope_resolution=**17** (README says 24, off-by-7)
  - Total: 20 + 36 + 26 + 17 = **99** ✅ (total matches; per-module breakdown wrong on 3 of 4 modules)
- **Implicit knowledge items (top 3)**:
  1. **`HirParam` duplication** — `HirFnSig.inputs: Vec<HirParam>` AND `Body.params: Vec<HirParam>` both carry the same data (clone, rustc-style); flagged in `deep-review-r37.md` as R37 condition 1, only mentioned in `architecture-decisions.md` ADR-001. NOT in `05-ast.md` or `06-mir.md` design docs.
  2. **HIR/AST sharing ratio (B3 higher than design)** — `05-ast.md` §13.2 retroactively notes "HIR 与 AST 共享更多类型（如 `Path`、`Ident`、`Visibility`）", but the original Stage 1.1 design plan (`plan-1.1.md` §C4) said "HIR 与 AST 共享约 50% 结构"; the actual sharing ended up higher. The deviation was accepted at Stage 6.18 write-back but the Stage 1.1 plan vs impl delta is not in design docs.
  3. **`HirTy::TraitObject` planned since Stage 1.1** — `plan-1.1.md:217` explicitly listed `TraitObject` as one of 16 planned `HirTy` variants; the AST (`ast/kinds.rs:246`) and HIR (`hir/kinds.rs:536`) both implement it. The fact that the MIR-level `TyKind` (defined at Stage 2.1) omitted `Dynamic` is the TD-029 root cause. This Stage 1.1 plan reference is critical context for TD-029 framing but is NOT in any §25.8 write-back.
- **Design doc §25.8 coverage**: ⚠️ **partial — retroactive only**. `05-ast.md` §13 (Stage 6.18) covers HIR data structures retroactively. Main body of `05-ast.md` has **zero** references to Stage 1.1-1.4 implementation work. `03-type-system.md` §10 (Stage 6.18) covers type system deviations but does not mention HIR-level `TraitObject` implementation.
- **New findings vs r216**: 3 of 4 module test counts in README are wrong (but total is correct).
- **Revised stage-round attribution**: Stage 1.1 is where `TraitObject` first appeared as a planned variant — this is the **Stage 1 design origin** of the type, making TD-029's MIR-level omission traceable to a Stage 1→2 transition gap (HIR has it, MIR doesn't).

### Stage 2 Re-audit (r217)

- **Test count verification**:
  - README claim (`docs/tests/v0/stage2/plan/README.md`): 141 total = mir_lowering 45 + negative_cases 30 + integration 35 + typeck_borrowck 31
  - Actual `#[test]` count: integration=**58** (README says 35, off-by-23), mir_lowering=**22** (README says 45, off-by-23), negative_cases=**35** (README says 30, off-by-5), typeck=**26** (README says 31, off-by-5; actual filename is `typeck_tests.rs` not `typeck_borrowck_tests.rs`)
  - Total: 58 + 22 + 35 + 26 = **141** ✅ (total matches; per-module breakdown wrong on all 4 modules)
- **Implicit knowledge items (top 3)**:
  1. **MIR `TyKind` initial variant count = 16** (Stage 2.1), now 17 after Stage 4.4 added `Closure` — only in Stage 2 dev-log §2.1. The fact that the original 16 omitted `Dynamic` (now TD-029) is implicit here. Should be in `06-mir.md` §14 §25.8 write-back.
  2. **NLL P0-1 through P0-17** — 17 P0 bugs found and fixed in Stage 2.4a-d (TyVid sharing, array lengths hardcoded, projections never constructed, etc.) — only in `gate-review-final.md`, not in `04-ownership-borrowing.md` design doc. The 17 P0 list is a critical historical record for understanding why NLL implementation took 6 gate-review rounds.
  3. **§16 compliance via pre-computed `FieldTyTable` + `FnSigTable`** — the architectural decision to make `check_mir_body_with_tables` read zero HIR by pre-computing field types and fn sigs in driver, only in Stage 3.60 retroactive update notes (referenced from Stage 2 dev-log). This is the **foundational §16 compliance pattern** that all later stages build on, but it's only documented in retroactive notes, not in `03-type-system.md` or `06-mir.md` design docs.
- **Design doc §25.8 coverage**: ⚠️ **partial — retroactive only**. `06-mir.md` §14 (Stage 6.11) covers MIR types retroactively (17 refs to Stage 0-4 in main body — most of any design doc). `03-type-system.md` §10 (Stage 6.18) covers type system. `04-ownership-borrowing.md` §11 (Stage 6.18) covers NLL. However, **`06-mir.md` §14 does NOT explicitly note the Stage 2.1 `TyKind` variant omission of `Dynamic`** — this is the TD-029 write-back gap.
- **New findings vs r216**: All 4 module test counts in README are wrong (but total is correct).
- **Revised stage-round attribution**: **TD-029 root cause is Stage 2.1** — this is where `TyKind` was first defined with 16 variants (omitting `Dynamic`). The Stage 5 dyn Trait work (5.61-5.80) built on this incomplete MIR type by using `DynTraitFatPtr` side-table as a workaround. The §25.8 write-back should explicitly note "Stage 2.1 MIR types definition omitted `Dynamic` variant; Stage 5 worked around this with `DynTraitFatPtr` side-table; TD-029 closes the gap by adding `Dynamic` to `TyKind` and refactoring `DynTraitFatPtr` to internal representation."

### Stage 3 Re-audit (r217)

- **Test count verification**:
  - README claim (`docs/tests/v0/stage3/plan/README.md`): 309 total = codegen_tests.rs (implied 309)
  - Actual `#[test]` count: codegen_tests=**294**, deep_inspection=**15** (NOT mentioned in README)
  - Total: 294 + 15 = **309** ✅ (total matches; deep_inspection_tests.rs missing from README module table)
  - Conformance 03-codegen: 601 ✅
- **Implicit knowledge items (top 3)**:
  1. **Stage 3.56-3.60 §16 compliance refactor** — the 5 sub-stages that introduced `FieldTyTable`, `FnSigTable`, `body_metas`, `fn_name_by_def_id` as pre-computed data tables to eliminate typeck/codegen HIR reads. Only in Stage 3 dev-log sub-stages 3.56-3.60 entries. This is the **architectural foundation** of §16 compliance but is not in `07-codegen.md` §25.8.
  2. **L1 PHI optimization rejected as design decision** (Stage 4.2) — the decision to rely on LLVM `mem2reg` pass rather than emit PHI nodes directly; rooted in Stage 3.4 Emitter trait design. Only in Stage 4 `dev-log.md` §4.2; should be in `07-codegen.md` §25.8 as accepted B3 deviation.
  3. **TextEmitter locals cache reset at block boundaries** (Stage 3.22) — the bug fix where `emit_block` clears `self.locals` at each block boundary to prevent `if x > 0 { 1 } else { 2 }` returning `2` regardless of `x`. Only in Stage 3 dev-log §3.22; should be in `07-codegen.md` §25.8 as "block-scoped local value cache" implementation note.
- **Design doc §25.8 coverage**: ⚠️ **partial — retroactive only**. `07-codegen.md` §14 (Stage 6.11) covers codegen retroactively (2 refs to Stage 0-4 in main body — Stage 3.49 fat pointer + Stage 3.4 Emitter trait). `07-codegen.md` §15 (Stage 8.6) covers Stage 8 async/await. None of the Stage 3.1-3.69 incremental codegen hardening (3.10-3.19) is in the design doc.
- **New findings vs r216**: `deep_inspection_tests.rs` (15 tests) missing from README module table.
- **Revised stage-round attribution**: TD-028 (mir::dyn_trait §16 violation) **architectural root cause traces to Stage 3.4** — where the Emitter trait was first made `pub` and callable from any module. The Stage 5.63-5.74 emit_* functions reused this Stage 3.4 pattern across the MIR→codegen boundary. The §25.8 write-back should note: "Stage 3.4 Emitter trait is `pub` for codegen-internal use; Stage 5.63-5.74 violated §16 by reusing it from `mir::dyn_trait`. Fix: relocate the 7 emit_* functions to `codegen/trait_dispatch.rs`."

### Stage 4 Re-audit (r217)

- **Test count verification**:
  - README claim (`docs/tests/v0/stage4/plan/README.md`): 13 total = closure_call 4 + closure_capture 3 + module 4 + macro 2
  - Actual `#[test]` count: closure_call=**2** (README says 4, off-by-2), closure_capture=**4** (README says 3, off-by-1), closure_full_call=**2** (NOT mentioned in README), macro_system=**3** (README says "macro 2", off-by-1; actual filename is `macro_system_tests.rs` not `macro_tests.rs`), visibility=**2** (NOT mentioned in README; README lists "module 4" but no `module_tests.rs` file exists — actual file is `visibility_tests.rs`)
  - Total: 2 + 4 + 2 + 3 + 2 = **13** ✅ (total matches; per-module breakdown wrong + README references nonexistent `module_tests.rs`)
- **Implicit knowledge items (top 3)**:
  1. **L1 PHI optimization rejected** (Stage 4.2) — decision documented in Stage 4 `dev-log.md` §4.2 but NOT in `07-codegen.md` §25.8. The 4 reasons (mem2reg well-tested, duplicate work, alloca-IR correct, opt non-blocking) are only in dev-log.
  2. **Closure call lowering deferred** (Stage 4.4) — Stage 4.4 added closure TYPE lowering (`AggregateKind::Closure` + `TyKind::Closure`) but explicitly deferred call dispatch. The deferral note is in `src/mir/lower/expr_operand.rs:876` code comment AND Stage 4 dev-log §4.4. This is the **root cause of TD-030**. Should be in `06-mir.md` §14 §25.8 and `07-codegen.md` §15 §25.8.
  3. **Macro expansion: 7 of 26 hardcoded** (Stage 4.10) — only in Stage 4 dev-log §4.10; the gap to 26 (per `13-stage1-feature-whitelist.md` §2.6) is NOT in any design doc §25.8. The decision to hardcode (rather than implement `macro_rules!`) was a Stage 4.10 scope decision; the full 26-macro Stage 1 contract is in `13-stage1-feature-whitelist.md` but the Stage 4.10 partial-impl status is not written back to `07-codegen.md` §15.
- **Design doc §25.8 coverage**: ⚠️ **partial — retroactive only**. `04-ownership-borrowing.md` §11 (Stage 6.18) covers closure capture. `07-codegen.md` §15 (Stage 8.6) covers closure codegen. Neither mentions the 7/26 macro hardcoding or the closure-call-lowering deferral.
- **New findings vs r216**: README references nonexistent `module_tests.rs` (actual: `visibility_tests.rs`); 3 of 4 module test counts wrong (but total correct).
- **Revised stage-round attribution**:
  - **TD-030 (closure call lowering)** traces to **Stage 4.4** — where closure type lowering was added but call dispatch was explicitly deferred. The deferral is in the code comment `src/mir/lower/expr_operand.rs:876` ("Closure call lowering: closure calls still go through regular Call").
  - **TD-032 (macro_rules!)** traces to **Stage 4.10** — where 7 hardcoded macros were added with no `macro_rules!` subsystem. The 19 missing macros are the Stage 4.10 scope deferral.

---

## 4. Stage 12 vs Stage 13 Reframing

### 4.1 Stage 12 proper scope (since Stage 13 launch is premature)

The user clarification is that **we are at Stage 12 (just started)**, NOT Stage 13. The r216
audit and Stage 13 plan creation in the previous turn were premature in framing Stage 12 as
"complete" and Stage 13 as "launched". The corrected Stage 12 scope:

| Sub-stage | Topic | Status | Notes |
|-----------|-------|--------|-------|
| 12.1 | v0.1 release + v0.3 bootstrap prep | ✅ Done (Stage 12.1) | v0.1-release.md + v0.3-bootstrap-prep.md produced |
| 12.2 | First-pass cross-stage audit (r216) | ✅ Done | 2 audit reports (architecture 350 lines + techdebt 650 lines) |
| **12.3** | **Second-pass review (this audit r217)** | 🔄 **Current** | Stage round revision + Stages 0-4 re-audit + Stage 12/13 framing correction |
| 12.4 | r216/r217 revision corrections | ⏳ Pending | Fix TD-030/031/032 numeric errors in r216 docs; correct Stage 0-4 README per-module test counts; correct Stage 4 README `module_tests.rs` → `visibility_tests.rs` |
| 12.5 | Cargo.toml version revert | ⏳ Pending | Revert v0.22.0 → v0.21.2 (see §4.3) |
| 12.6 | Reframe `plan-13.1.md` | ⏳ Pending | Move to Stage 12 as "future-stage planning" output (see §4.2) |
| 12.7 | §25.8 implicit-knowledge backfill | ⏳ Pending | Write back 15 implicit-knowledge items identified in §3 to design docs |
| 12.8 | Stage 12 final gate review | ⏳ Pending | Close Stage 12 before launching Stage 13 |

**Stage 12 exit criteria** (proposed):
1. ✅ All 5 TD numeric/framing corrections applied to r216 docs (or r217 supersedes r216)
2. ✅ Cargo.toml version = v0.21.2 (patch bump from v0.21.0)
3. ✅ `plan-13.1.md` reframed as Stage 12 output (not Stage 13 launch)
4. ✅ Stage 0-4 README per-module test counts corrected
5. ✅ Stage 12 gate review document produced
6. ✅ Stage 13 launch explicitly NOT started (no Stage 13 sub-stage work begins until Stage 12 closes)

### 4.2 Stage 13 plan repositioning

The existing `docs/develop/v0/stage-13/plan-13.1.md` was prematurely created in the previous
turn (treated as "Stage 13 launched"). Three repositioning options:

| Option | Description | Pros | Cons | Recommendation |
|--------|-------------|------|------|----------------|
| (a) Keep as-is | Plan remains "Stage 13.1 launch plan", Stage 13 begins immediately | None — premature launch | Skips Stage 12 close; commits to v0.22.0 version policy; carries r216 numeric errors into Stage 13 work | ❌ Reject |
| (b) **Move into Stage 12 as "future-stage planning"** | Plan is reframed as Stage 12.6 output: "Stage 13 launch plan, to be executed when Stage 12 closes". Document lives in `docs/develop/v0/stage-12/` (e.g., `stage-13-launch-plan.md`) or remains in `stage-13/` but with header noting "Status: Draft, awaiting Stage 12 close" | Per §15 long-term > short-term: keeps valuable TD analysis work; allows Stage 13 to start with corrected r217 audit data; preserves optionality | Requires header/POV edit; some links may break | ✅ **RECOMMENDED** |
| (c) Delete | Plan removed; Stage 13 planning restarts in Stage 12.4+ | Forces fresh plan with corrected data | Loses valuable TD-030/031/032 closure analysis (5+ MUVs of work) | ❌ Reject |

**Recommendation: Option (b)**. Per §15.1 "When facing a choice between minimal-change and
optimal-architecture, choose optimal-architecture" — Option (b) preserves the planning work
(value) while correcting the framing (architecture). The plan should:
1. Be moved (or symbolically linked) to `docs/develop/v0/stage-12/` as `stage-13-launch-plan.md`
2. Have its header updated from `🔄 Planned` to `📋 Draft (Stage 12 output, awaiting Stage 12 close)`
3. Have its `Task ID: stage13.1-muv*-r217` references updated to reflect that r217 is the audit
   round, not the implementation round (implementation round will be r218+ when Stage 13 launches)
4. Be cross-referenced from `docs/develop/v0/stage-12/README.md` as a Stage 12 deliverable

The `docs/develop/v0/stage-13/` directory can remain (as the future Stage 13 home), but the
plan-13.1.md file's header should make clear it's a Stage 12 planning artifact, not Stage 13 work-in-progress.

### 4.3 Version policy

**Current state**: `Cargo.toml` version = `0.22.0` (bumped from v0.21.0 in the previous turn,
treating Stage 12.2 as a minor-version release).

**User request**: v0.21.2 (patch bump).

**Semver analysis**:

| Bump type | Version | Justification | Applicable? |
|-----------|---------|---------------|-------------|
| Patch (0.21.x) | v0.21.2 | Bug fixes / metadata / docs only — no new compiler features, no API changes | ✅ **YES** — Stage 12.2/12.3 added only docs (audit reports + plan + READMEs) and 10 verification tests; zero compiler behavior changes |
| Minor (0.x.0) | v0.22.0 | New features / API additions — backward-compatible | ❌ NO — no new compiler features; the Cargo.toml `description` field was expanded (Stage 5.1-7.8 history appended) but that's metadata, not functionality |
| Major (x.0.0) | v1.0.0 | Breaking changes | ❌ NO — no breaking changes |

**Recommendation: v0.21.2 (patch bump)**. Per semver §2.0.0:
- "Patch version Z (x.y.Z | x > 0) MUST be incremented when only backwards compatible bug fixes are introduced."
- Stage 12.2/12.3 work is documentation + audit + test additions, not feature work.
- The Stage 12.2 deliverables (audit reports, README backfill, plan-13.1.md) are project
  artifacts, not compiler features.

**Action**: Revert `Cargo.toml` `version = "0.22.0"` → `version = "0.21.2"`. Update the
`description` field to reflect the actual baseline (currently the description ends at Stage
7.8 — should reflect Stage 12 audit work as "Stage 0-12 complete + cross-stage audit r216/r217").

**Version history correction**:
- v0.21.0: Stage 12.1 (v0.1 release + v0.3 bootstrap prep) — shipped
- v0.21.1: (skipped — no intermediate release)
- **v0.21.2: Stage 12.2 + 12.3 (cross-stage audit r216 + second-pass r217 + D7 backfill + Stage 13 plan draft)** — target

---

## 5. Recommendations for Stage 12.3+ Planning

### Priority P0 — Must close before Stage 13 launch

1. **Apply TD numeric corrections to r216 docs** (or supersede with r217): Fix TD-030 (0 FAIL
   markers in closure dirs, not 41), TD-031 (11 if-let/while-let FAIL tests, not 12, in
   `00-parse/02-control-flow/` not `02-borrowck/01-nll-advanced/`), TD-032 (7 of 26 hardcoded,
   not 26 hardcoded; 19 missing). Estimated 1-2 hours.

2. **Revert Cargo.toml version** v0.22.0 → v0.21.2. Estimated 5 minutes.

3. **Reframe `plan-13.1.md`** as Stage 12 output (header change + cross-reference from
   stage-12 README). Estimated 30 minutes.

4. **Correct Stage 0-4 README per-module test counts** (5 README files; fix off-by-N counts
   in 4 of 5 READMEs; fix nonexistent `module_tests.rs` reference in stage4 README). Estimated
   1-2 hours.

### Priority P1 — Should close during Stage 12

5. **§25.8 implicit-knowledge backfill** for 15 items identified in §3 (3 per stage × 5 stages).
   Each write-back is 5-20 lines in the target design doc. Estimated 4-6 hours total.

6. **Expand TD-029 §25.8 write-back scope** to include `06-mir.md` §14 (currently only
   `03-type-system.md` §10/§11/§12 is in scope). Add note about Stage 2.1 root cause and
   AST/HIR already implementing `TraitObject`. Estimated 1 hour.

7. **Resolve r216 internal inconsistency** on TD-031 (architecture doc says 6+5=11 in
   `02-borrowck/01-nll-advanced/`; techdebt doc says 12 in `00-parse/02-control-flow/`).
   r217 verifies actual = 11 in `00-parse/02-control-flow/`. Update both r216 docs to
   cite the correct number AND location. Estimated 30 minutes.

### Priority P2 — Should close before Stage 13.1 implementation

8. **Add Stage 3.4 Emitter trait root-cause note** to TD-028 §25.8 write-back in `06-mir.md` §14.
   The §16 violation was enabled by Stage 3.4's `pub` Emitter trait; Stage 5.63-5.74 reused
   it across the MIR→codegen boundary. Estimated 30 minutes.

9. **Update `src/mir/lower/expr_operand.rs:876` code comment** to reflect r217 verification
   (the comment says "closures still go through regular Call" — accurate; should add
   "TD-030 P0 blocker, see r217 audit for FAIL test count clarification"). Estimated 15 minutes.

10. **Stage 12 final gate review document** — closes Stage 12 before Stage 13 launch. Estimated 2-4 hours.

### Priority P3 — Optional, can defer to Stage 13

11. **Update Cargo.toml `description` field** to reflect Stage 8-12 history (currently truncated
    at Stage 7.8). Estimated 30 minutes.

12. **Backfill `docs/tests/v0/stage5/plan/README.md` cross-reference** from Stage 12.2 D7 backfill
    (the r216 techdebt audit reports this was done — verify and link from stage-12 README).

---

## 6. Committee Vote (ARCH-A + REV-A)

### **GO-WITH-CONDITIONS**

**Reasoning**:

The r217 second-pass audit confirms that the r216 first-pass audit was **directionally
correct** (5 TD items identified, all real gaps) but **numerically and methodologically
imperfect**:

- ✅ TD-028: r216 attribution CORRECT (verified 7 emit_* functions in 5.63-5.74 range, ≤3 files fix scope)
- ⚠️ TD-029: r216 framing INCOMPLETE (NEW at MIR level only; AST/HIR implement TraitObject; root cause is Stage 2.1, not Stage 5)
- ❌ TD-030: r216 numerically WRONG (0 `//! FAIL` markers in cited dirs, not 41; methodology error conflating FAIL with compile_error)
- ❌ TD-031: r216 numerically WRONG + internally inconsistent (11 actual, not 12; wrong directory cited in architecture doc)
- ❌ TD-032: r216 framing INVERTED (7 of 26 hardcoded, not 26 hardcoded; 19 missing, not 26 implemented)

The Stages 0-4 re-audit confirms all 5 stage README files have **correct total test counts**
(344, 99, 141, 309, 13) but **incorrect per-module breakdowns** in 4 of 5 READMEs — a
documentation defect that should be corrected in Stage 12.4.

The §25.8 design doc coverage analysis reveals a **systematic implicit-knowledge gap**: 4 of
6 design docs have **zero** references to Stage 0-4 implementation work in their main body,
relying entirely on retroactive Stage 6.18/8.6 write-backs. This is acceptable as a
historical artifact but should be explicitly noted in the Stage 12 final gate review.

**Conditional GO**: Stage 12.3 (this audit) is **ratified** as a complete second-pass review.
Stage 13 launch is **NOT authorized** until Stage 12.4-12.8 close the 5 TD numeric/framing
corrections, version revert, plan reframing, README corrections, and final gate review.

**Specific conditions for Stage 12 close**:
1. ✅ r217 audit report produced (this document)
2. ⏳ r216 docs updated with r217 corrections (or r216 marked superseded by r217)
3. ⏳ Cargo.toml version reverted to v0.21.2
4. ⏳ `plan-13.1.md` reframed as Stage 12 output
5. ⏳ Stage 0-4 README per-module test counts corrected
6. ⏳ Stage 12 final gate review document produced

**For v0.1 release**: ✅ Still RATIFIED — r217 does not affect v0.1 release artifact
(5026/5000 conformance gate holds; the r216 numeric errors are in audit documentation, not
in the conformance test suite itself).

**For v0.3 self-hosting target**: ⚠️ CONTINGENT on Stage 13 executing the corrected TD-030,
TD-031, TD-032 closure plan (with r217-verified numbers) — same conditional as r216, but
with corrected baseline data.

---

**Audit completed**: 2026-07-26
**Companion audits**:
- `cross-stage-audit-r216-architecture.md` (ARCH-A, D1+D5, GO-WITH-CONDITIONS) — to be updated with r217 corrections
- `cross-stage-audit-r216-techdebt-tests-docs.md` (combined, D2+D3+D4+D6+D7, GO-WITH-CONDITIONS) — to be updated with r217 corrections

**Next action**: Stage 12.4 — apply r217 corrections to r216 docs (or supersede) + Cargo.toml version revert + plan-13.1.md reframe + Stage 0-4 README corrections.
