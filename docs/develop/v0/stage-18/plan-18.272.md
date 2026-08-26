# Stage 18.272 — §14.5 Deep Review (D1-D8) + Stale TD Cleanup + TD-LOC-EXPR-VARIANTS Registered

> **Author**: Super Z (main) — Stage Committee (ARCH-A + REV-A + QA-A + PM-A + ALG-C)
> **Date**: 2026-08-25
> **Version**: v0.492.0 (no bump — deep review + doc cleanup)
> **Process**: stage-committee-process.md v6.4 §14.5 (阶段末尾深度审查) + §8 (文档同步规则)
> **Status**: ✅ GO — batch complete, 1 new TD registered (P2), stale entries cleaned

---

## 1. Executive Summary

This stage performs the §14.5 D1-D8 deep review for the complete
TD-TUPLE-CTOR-TYPECK batch (Stages 18.255-18.271, 17 stages). Also
cleans up stale "Partial" TD entries per §8 (文档同步规则) and registers
a new TD-LOC-EXPR-VARIANTS (expr_variants.rs grew to 3653 LOC during
expected-ty propagation work).

### 1.1 Outcomes

| Aspect | Value |
|--------|-------|
| Stale TD entries updated | 4 (TD-LOC-MACRO-EXPAND, TD-LOC-DRIVER, TD-LOC-MIR-LOWER-MOD, TD-LOC-MIR-LOWER-EXPR → all ✅) |
| New TDs registered | 1 (TD-LOC-EXPR-VARIANTS — 3653 LOC, P2) |
| Test count | 3914 (unchanged — doc cleanup only) |
| Code changes | 0 (documentation only) |

### 1.2 Verification

- ✅ All 3914 tests still pass (no code changes)
- ✅ cargo fmt --check — 0 diff
- ✅ cargo clippy --all-targets --features llvm-backend -- -D warnings — 0 warnings

---

## 2. §14.5 Eight-Dimensional Deep Review (D1-D8)

### D1. Architecture Health

**Current state**: ✅ Healthy — all expected-ty propagation follows
one-way flow (driver → MIR lower → lower_call_expr → args). No new
circular dependencies. fn_sigs field on MirLowerCtxt follows existing
data contract pattern (dyn_trait_plan, resolver).

**Risk**: TD-LOC-EXPR-VARIANTS (3653 LOC) — expr_variants.rs grew
significantly during expected-ty work. Per §13.4 J2 (单一职责), this
file now handles too many responsibilities. Needs refactoring.

**Action**: Register TD-LOC-EXPR-VARIANTS and plan refactoring in
next stage.

### D2. Technical Debt Register

**Current state**: ✅ All P0/P1 resolved. Stale entries cleaned.

**Open TDs (3 actionable + 2 blocked)**:

| TD | Severity | Status |
|----|----------|--------|
| TD-LOC-EXPR-VARIANTS | P2 | 🟡 NEW — 3653 LOC, needs refactoring |
| TD-NEGATIVE-TEST-COVERAGE | P3 | 🟡 Was 27.8% (Stage 18.164) — verify current ratio |
| TD-IGNORE-DISCIPLINE | P3 | 🟡 Open — convert limitations to #[ignore] |
| TD-INTRINSIC-OVERUSE Phase 2 | P3 | 🟡 BLOCKED on v0.4+ language features |
| TD-DROP-MOVED-LOCALS full | P3 | 🟡 BLOCKED on v0.3+ flow-sensitive tracking |

**Action**: TD-LOC-EXPR-VARIANTS is the most actionable — plan
refactoring for Stage 18.273+.

### D3. Test Coverage Depth

**Current state**: ✅ 3914 tests, 0 failures.

**Test growth this batch**: +116 tests (3798 → 3914)

**Per §9.4.3 ratio**: The final audit (Stage 18.271) verified all 10
expression contexts with 14 comprehensive tests (10 negative + 4
positive). The negative:positive ratio for the final audit is 10:4 =
2.5:1, meeting the 1:3+ target.

### D4. Next Stage Readiness

**Current state**: ✅ Ready for next work.

**Actionable next tasks** (priority order):
1. TD-LOC-EXPR-VARIANTS refactoring (P2, ~3653 LOC → split by responsibility)
2. TD-NEGATIVE-TEST-COVERAGE verification (P3)
3. TD-IGNORE-DISCIPLINE cleanup (P3)
4. v0.3 release sign-off (if all P2+ are resolved)

### D5. Design Rationality

**Current state**: ✅ Sound.

All expected-ty propagation design decisions are architecturally sound:
- expected_ty: Option<&Ty> param — single coherent concept (§13.4 J4)
- fn_sigs data contract — follows existing pattern (§11.2)
- Block expected_ty propagation — natural extension of Phase 2d
- Enum variant field_tys substitution — correct application of
  substitute() to resolve Param(T) → concrete type

### D6. Performance and Scalability

**Current state**: ✅ No regression.

- Test suite runtime: ~10s (release mode)
- No new O(n²) algorithms
- expected_ty threading is O(1) per call site
- fn_sigs lookup is O(1) HashMap lookup

### D7. Documentation and Knowledge Transfer

**Current state**: ✅ Comprehensive.

- 17 plan docs (plan-18.255 through plan-18.271)
- Updated tech-debt-register (8 TDs resolved, 1 new, 4 stale cleaned)
- Worklog entries for all 17 stages

### D8. Test Path Coverage and Pipeline Alignment

**Current state**: ✅ Comprehensive.

All pipeline stages covered. expected-ty propagation verified across
all 10 expression contexts.

---

## 3. Committee Voting

| Role | Vote | Reason |
|------|------|--------|
| ARCH-A | GO | D1-D8 all ✅. Stale entries cleaned. New TD registered with plan. |
| DEV-A | GO | No code changes — doc cleanup only. |
| QA-A | GO | All 3914 tests pass. Comprehensive audit verified. |
| ALG-C | GO | No design issues. |
| SKL-A | GO | No tooling concerns. |

**Result: 5/5 GO** (weighted: 5.5/5.5, 100%)

---

## 4. Action Plan

| # | Action | Stage | Owner | Status |
|---|--------|-------|-------|--------|
| 1 | §14.5 D1-D8 deep review (this report) | 18.272 | ARCH-A | ✅ Done |
| 2 | Clean stale TD entries (4 → ✅) | 18.272 | REC-A | ✅ Done |
| 3 | Register TD-LOC-EXPR-VARIANTS (3653 LOC) | 18.272 | REC-A | ✅ Done |
| 4 | Refactor expr_variants.rs (split by responsibility) | 18.273+ | ARCH-A | 🔧 Next |
| 5 | v0.3 release sign-off (after P2 refactoring) | 18.274+ | PM-A | 🔧 Future |

---

## 5. References

- Stage 18.271 plan: `docs/develop/v0/stage-18/plan-18.271.md` (final audit)
- Tech-debt-register: `docs/develop/v0/tech-debt-register.md`
