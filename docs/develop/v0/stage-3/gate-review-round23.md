# Stage 3 Phase Gate Review — Round 23

> **Author**: redskaber
> **Date**: 2026-07-21
> **Process**: v3.13 (§18 round-completion doc sync)
> **Stage baseline**: v0.8.6 (Stage 3.55 — void function return type fix)
> **Audit tool**: `examples/stage3_gate_audit_r23.rs`
> **Prior rounds**: R1-R22 all CONVERGED

---

## 1. Audit Design

R23 covers Stage 3.56 — **Pipeline architecture refactoring (Phase A)**.
Codegen is now a pure MIR consumer — it reads pre-built MIR from
`CompileResult` instead of re-lowering HIR→MIR + re-running typeck.

This is an **architectural** change, not a feature addition. The R23 audit
(30 cases) validates that the refactored pipeline produces identical IR
output, plus validates the new architecture contracts (pre-built MIR,
precomputed metadata, §16 compliance).

| Group | Cases | Purpose |
|-------|-------|---------|
| R — Regression (8) | Re-verify R22 cases (void fn, &str param, &[i64], enum, &str index, slice field, const, i16) |
| A — Architecture contracts (14) | Pure MIR consumer, no double lowering, void from MIR, fn_name precomputed, body_metas parallel, complex pipeline, §16 compliance regression (7) |
| E — §9.3.2 edge cases (8) | void empty, void with params, non-void i32/str, multiple fns, struct return, array param, while loop |
| **Total** | **30** |

---

## 2. Audit Execution

```
✅ AUDIT PASSED — 0 codegen defects found in 30 cases.
   R1-R23: ..., 28, 28, 23, 24, 30, 30, 30, 30/30 — all OK.
   Per §9.3.3, audit CONVERGED (23 rounds, 0 new issues each).
   Stage 3.56 (pipeline architecture refactoring: codegen as pure MIR consumer) verified.
```

---

## 3. Stage 3.56 Summary — Pipeline Architecture Refactoring (Phase A)

### Problem

Codegen (`codegen_crate_with_emitter`) took `&HirCrate` and re-ran:
1. `crate::mir::lower::lower_hir_body_to_mir_full` (Stage 2.1 internal)
2. `crate::typeck::TypeChecker::check_mir_body_with_hir` (Stage 2.2 internal)
3. `crate::driver::owner_return_ty_for_body` (reverse dep: Stage 3 → driver)

This violated §16.1/§16.3 (Stage 3 calling Stage 2 internals). It also:
- Silently skipped borrowck (errors were in `CompileResult` but codegen ignored them)
- Dropped type errors (typeck results were in `CompileResult` but codegen ignored them)
- Made `CompileResult.mirs` and `CompileResult.typeck_results` dead data
- Did O(n²) work (re-lowering every body, re-scanning hir.owners per body)

### Fix

1. **`src/driver.rs`**: Added `BodyMeta { fn_name, is_void, param_count }` and
   `fn_name_by_def_id: HashMap<DefId, String>` to `CompileResult`. `compile()`
   pre-computes these during the pipeline run.

2. **`src/codegen/mod.rs`**: `codegen_crate` now takes `&CompileResult`. New
   `codegen_from_mir(mirs, body_metas, fn_name_by_def_id, interner, emitter)`
   is the §16-compliant entry point. Zero HIR references, zero calls to
   upstream stage functions.

3. **Updated 270+ call sites**: `gen_ll` helper + all audit scripts.

### §16 Compliance Verification

```
$ grep -n "crate::mir::lower\|crate::typeck" src/codegen/mod.rs
(no function call matches — only doc comments and data type references)
```

### Plan Agent Audit

The Plan agent identified 8 issues (P0-1 through P2-8). This stage
addresses **P0-1** (codegen re-lowers), **P0-2** (codegen reaches into
upstream stages), and **P1-7** (O(n²) owner lookup). Remaining issues
(P0-3 glob exports, P0-4 Stage trait, P1-5 Emitter test, P1-6 error path,
P2-8 error model) are deferred to Phase B-D.

---

## 4. Committee Vote: 5/5 APPROVED — UNANIMOUS

---

## 5. Updated Limitation List

| ID | Status |
|----|--------|
| Codegen re-lowering + §16 violations (architectural) | **CLOSED in Stage 3.56** ✅ |
| All prior CLOSED items | ✅ |
| Remaining open: L1 (PHI), L3 (closures), L5 (traits), L8 (lli), L-COPY-ADT, P0-3 (glob exports), P0-4 (Stage trait), P1-5 (Emitter test), P1-6 (error paths), P2-8 (error model) | Open |

---

## 6. Test Progression

| Version | Tests | Delta |
|---------|-------|-------|
| v0.8.6 (3.54, field indexing fix) | 938 | +9 |
| v0.8.6 (3.55, void fn fix) | 947 | +9 |
| **v0.8.6 (3.56, Phase A refactoring)** | **953** | **+6** |

---

## 7. §18 Document Sync Compliance (process v3.13)

| Document | Status |
|----------|--------|
| `docs/develop/v0/stage-3/dev-log.md` | ✅ Stage 3.56 entry added (before Test Progression, no duplicate) |
| `docs/develop/v0/stage-3/gate-review-round23.md` | ✅ This file |
| `docs/tests/matrix.md` | ✅ Updated (953 tests, 686 cumulative audit cases) |
| `README.md` | ✅ Updated (953 tests, 23 rounds) |
| `examples/stage3_gate_audit_r23.rs` | ✅ Created (30 cases) |
| `worklog.md` | ✅ Stage 3.56 entry appended |

---

## 8. Conclusion

Stage 3 Round 23 **PASSED**. Codegen is now a pure MIR consumer —
zero calls to upstream stage functions, no double lowering, no double
typeck, no skipped borrowck, no dropped type errors. This is the most
significant architectural improvement since Stage 3.47 (L-PIPE-1 closure).

**Phase B-D deferred**: glob export cleanup (P0-3), Stage trait (P0-4),
Emitter pluggability test (P1-5), error path coverage (P1-6), unified
error model (P2-8). These are lower-risk and can be addressed in future
stages.
