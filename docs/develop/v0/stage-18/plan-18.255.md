# Stage 18.255 — TD-TUPLE-CTOR-TYPECK Infrastructure Audit + Phase 1 Fix + Phase 2 Design

> **Author**: Super Z (main) — Stage Committee (ARCH-A + REV-A + QA-A + PM-A + ALG-C)
> **Date**: 2026-08-25
> **Version**: v0.492.0 (no bump — Phase 1 small fix + Phase 2 design only)
> **Process**: stage-committee-process.md v6.4 §17 (task planning) + §13.1 (design alignment) + §13.4 (refactoring six judgments) + §13.5 (design-review cycle)
> **Status**: Phase 1 RESOLVED + Phase 2 DESIGN (deferred to Stage 18.256+)

---

## 1. Executive Summary

This stage audits the **TD-TUPLE-CTOR-TYPECK** soundness hole identified at
Stage 18.233, performs Phase 1 of the fix (low-risk, immediate), and produces
the complete architectural design for Phase 2 (deferred to Stage 18.256+).

### 1.1 Outcomes

| Phase | Scope | Status | LOC |
|-------|-------|--------|-----|
| Phase 1 | Unify arg order swap (Adt field + Array element) | ✅ Resolved | ~2 LOC + comments |
| Phase 2 | Expected-type propagation through MIR lower | 🔧 DESIGN ONLY | ~500 LOC planned |

### 1.2 Verification

- 10 new regression tests added (3 positive + 6 negative + 1 deferred-MVP)
- Total tests: **3811** (was 3798), 0 failures
- Per §9.4.3 1:3+ ratio: 3 positive + 6 negative = 1:2 (negative not yet
  3× positive — Phase 2 will add more negatives after fix lands)

---

## 2. §17.1 Step 1 — Scan Documents & Confirm Capability Boundary

### 2.1 Design Intent (per §13.1 alignment with lang-design)

| Design Doc | Section | Intent |
|-----------|---------|--------|
| `03-type-system.md` §3.2 | Tuple struct ctor | `Wrapper<T>(*mut T)` — ctor call `Wrapper::<i32>(arg)` must type-check arg against `*mut i32` |
| `06-mir.md` §16.8 | TD-TUPLE-CTOR-TYPECK | Listed as "deferred to v0.3" — root cause: temp local loses expected type context |
| `06-mir.md` §3.2 Rvalue | AggregateKind::Adt | Carries `field_tys: Vec<Ty>` — declared field types sunk into MIR |
| `06-mir.md` §16.5 | substitute(ty, substs) | Pure function for generic substitution — already implemented (Stage 16.53) |

**Design intent summary (3-5 sentences)**: Tuple struct ctor calls
(`Wrapper::<T>(arg)`) must validate that `arg`'s type matches the
declared field type after generic substitution. The field type flows
through MIR via `AggregateKind::Adt(_, _, substs, field_tys)`. Typeck
must unify each operand's type with the corresponding field_ty. When
turbofish is omitted but `let` annotation provides the expected type,
the typeck should propagate the expected type back to the ctor's args.

### 2.2 Capability Boundary (current state)

| Capability | Status | Evidence |
|-----------|--------|---------|
| Turbofish explicit type args (`Wrapper::<i32>(arg)`) | ✅ Works (since Stage 16.53) | substs propagate to Adt, field_tys substituted, unify happens |
| No turbofish + `let : Wrapper<i32>` annotation | 🔴 BROKEN | field_tys stay as `Param(T)`, unify silently accepts (Phase 2 fix needed) |
| Error message direction (turbofish case) | 🔴 REVERSED (was) | `unify(&op_ty, field_ty)` made "expected <actual>, found <declared>" |
| Generic substitution in field_tys | ✅ Works | `resolve_adt_field_tys_with_substs` (Stage 16.53) |
| Unify table handles Param ↔ concrete | ✅ Works (intentional) | `Param` is universally quantified, unifies with any concrete type |

### 2.3 Tech-Debt Status (per §6.2.1)

| TD ID | Severity | Status |
|-------|----------|--------|
| TD-TUPLE-CTOR-TYPECK | P2 | 🟡 Phase 1 resolved, Phase 2 deferred |
| TD-INTRINSIC-OVERUSE Phase 2 | P3 | 🟡 Blocked on v0.4 language features (primitive type impl, fat ptr construction) |
| TD-DROP-MOVED-LOCALS full | P3 | 🟡 Flow-sensitive tracking — independent of this TD |
| TD-EXPECT-TYPECK-SOLVER | P2 | 🟡 Open — 37 expect partial missing message (unrelated) |

---

## 3. §13.4 Six Judgments Audit (J1-J6)

The Phase 2 architectural change (expected-type propagation) must pass §13.4 six judgments:

| # | Judgment | Phase 2 (expected_ty threading) | Verdict |
|---|----------|-------------------------------|---------|
| J1 | Architecture alignment | Aligns with `06-mir.md` §3.2 (Rvalue/Aggregate carry complete type info) — currently field_tys are post-substitution, but expected_ty would be a separate orthogonal channel for call-site context. **Need design doc update** to add §3.3 "expected_ty propagation" section. | ✅ Pass with doc update |
| J2 | Single responsibility | Each `lower_expr_*` function gains one optional parameter. The threading is a single coherent concept (expected type context). | ✅ Pass |
| J3 | One-way flow (no cycles) | expected_ty flows: let-binding/return-stmt → lower_expr_to_operand → lower_call_expr → arg operands. No back-edges. | ✅ Pass |
| J4 | Compile-concept completeness | The "expected type" concept is a single semantic unit (like `span`). All `lower_expr_*` functions take it. No splitting across modules. | ✅ Pass |
| J5 | Stage division clear | Phase 2 only touches MIR lower (`src/mir/lower/`) + optionally HIR-to-MIR typeck integration. No codegen changes. Stage boundary respected. | ✅ Pass |
| J6 | Reasonable size | Each `lower_expr_*` function gets `+1 param` (small change). Total LOC: ~500 across 6 files. Within J6 envelope (mod.rs < 1500, submodules < 1500). | ✅ Pass |

**All 6 judgments pass** — Phase 2 is architecturally sound.

---

## 4. §17 Step 6 — Defect Integration (Phase 2 Plan)

### 4.1 Bug Verification Results (Stage 18.255 audit)

Tests added in `tests/v0/stage18/plan/stage18_255_td_tuple_ctor_typeck_regression_tests.rs`
verified the actual behavior:

| Test Case | Expected | Pre-Phase-1 Behavior | Post-Phase-1 Behavior |
|-----------|----------|----------------------|----------------------|
| `Holder::<i32>(42)` (turbofish + valid) | No error | ✅ No error | ✅ No error |
| `Holder(42)` with `let : Holder<i32>` (inferred + valid) | No error | ✅ No error | ✅ No error |
| `Pair::<i32, bool>(42, true)` (multi-field + valid) | No error | ✅ No error | ✅ No error |
| `Holder::<i32>(true)` (turbofish + wrong arg) | ERROR "expected i32, found bool" | 🔴 ERROR but message REVERSED | ✅ ERROR with correct message |
| `Holder::<i32>("hello")` (turbofish + str arg) | ERROR "expected i32, found ..." | 🔴 ERROR but message REVERSED | ✅ ERROR with correct message |
| `Wrapper::<i32>(true)` (turbofish + raw ptr field) | ERROR "expected *mut i32, found bool" | 🔴 ERROR but message REVERSED | ✅ ERROR with correct message |
| `Pair::<i32, bool>(42, 99)` (turbofish + wrong 2nd arg) | ERROR "expected bool, found {integer}" | 🔴 ERROR but message REVERSED | ✅ ERROR with correct message |
| `[1, true]` array mixed element | ERROR with correct direction | 🔴 REVERSED message | ✅ Correct message |
| `Holder(true)` with `let : Holder<i32>` (no turbofish + wrong arg) | ERROR (soundness hole) | 🔴 NO ERROR (soundness hole) | 🔴 NO ERROR (still soundness hole) — Phase 2 needed |

**Phase 1 fixed**: error message direction for turbofish + Array element cases.
**Phase 2 still needed**: soundness hole when no turbofish + `let` annotation provides expected type.

### 4.2 Phase 2 — Complete Fix Plan (deferred to Stage 18.256+)

#### 4.2.1 Root Cause

When `Holder(true)` is called without turbofish, MIR lower resolves
`Holder` to `Adt(def_id, [])` (empty substs). The field type stays as
`Param(T)`. The let-binding `let w: Holder<i32>` propagates `Holder<i32>`
to dest_local, but the `AggregateKind::Adt(def_id, [], [Param(T)])`
inside the rvalue doesn't see this expected type. So `unify(Param(T),
Bool)` silently succeeds (Param unifies with anything per Stage 18.54).

#### 4.2.2 Solution Architecture

Thread `expected_ty: Option<&Ty>` through MIR lower's `lower_expr_*`
functions:

```text
let w: Holder<i32> = Holder(true);
                    │              │
                    │              └─ lower_expr_to_operand(cx, ctor_expr,
                    │                  expected_ty = Some(Holder<i32>))
                    │                  │
                    │                  ├─ resolve Holder → Adt(def_id, [i32])
                    │                  │  (extracted from expected_ty,
                    │                  │   not from turbofish)
                    │                  │
                    │                  ├─ resolve_adt_field_tys_with_substs(
                    │                  │     cx, def_id, [i32]) → [i32]
                    │                  │
                    │                  └─ for each arg, lower with
                    │                     expected_ty = Some(field_tys[i])
                    │
                    └─ dest_local = Adt(def_id, [i32])
```

#### 4.2.3 Phased Implementation

| Phase | Stage | Scope | LOC | Dependencies |
|-------|-------|-------|-----|-------------|
| 2a | 18.256 | Add `expected_ty: Option<&Ty>` param to `lower_expr_to_operand` + `lower_expr_to_place`. Default `None` for all current call sites. | ~100 LOC | None (additive) |
| 2b | 18.257 | Thread expected_ty from `let : T = expr` (when T is concrete Adt). Lower the let-init expr with `expected_ty = Some(T)`. | ~80 LOC | 2a |
| 2c | 18.258 | Thread expected_ty into `lower_call_expr` Adt ctor path. Use expected_ty to extract substs when turbofish is absent. | ~150 LOC | 2b |
| 2d | 18.259 | Thread expected_ty from `return expr` (use fn return type). | ~50 LOC | 2c |
| 2e | 18.260 | Thread expected_ty into `lower_method_call_expr` for cases like `vec.push(Wrapper(true))` where the wrapper's expected type comes from method sig. | ~100 LOC | 2d |
| 2f | 18.261 | Negative regression tests for soundness hole (convert the deferred test from `stage18_255_*_regression_tests.rs`). | ~50 LOC | 2e |
| **Total** | 18.256-18.261 | 6 stages | ~530 LOC | sequential |

#### 4.2.4 §13.4 J1-J6 Re-validation per Phase

Each phase (2a-2f) must re-validate J1-J6 before implementation:
- J1: design doc update if architecture changes
- J2: ensure `expected_ty` doesn't bleed into non-expr contexts (e.g., statements)
- J3: no back-edges from `lower_call_expr` to caller
- J4: `expected_ty` is a single concept, not split
- J5: only touches MIR lower, no codegen/typeck changes
- J6: each stage's LOC change < 200 LOC

#### 4.2.5 Design-Review Cycle (§13.5)

Per §13.5, each phase 2a-2f must go through design-review cycle:
- ARCH-A produces design v1
- REV-A reviews (P0/P1/P2/P3 classification)
- ARCH-A revises until REV-A approves
- Maximum 5 iterations per phase

---

## 5. §17.7 Defect Integration — Same-Class Errors

Per §17.6, same-class errors should be considered holistically. During this
audit, I identified other unify arg order issues that share the same root
cause as Phase 1:

| Site | Current Call | Should Be | Severity |
|------|-------------|-----------|----------|
| `typeck/infer.rs:543` (Array elem) | `unify(&op_ty, elem_ty)` | ✅ FIXED to `unify(elem_ty, &op_ty)` | P2 |
| `typeck/infer.rs:568` (Adt field) | `unify(&op_ty, field_ty)` | ✅ FIXED to `unify(field_ty, &op_ty)` | P2 |
| `typeck/check.rs:314,340,392` (Call arg) | `unify(arg_ty, input_ty)` | Should be `unify(input_ty, arg_ty)` — same class | P3 |
| `typeck/check.rs:324,349,400` (Call return) | `unify(&dest_ty, &sig.output)` | Should be `unify(&sig.output, &dest_ty)` — same class | P3 |
| `typeck/check.rs:445` (Switch discr) | `unify(&discr_ty, &bool_ty)` | Should be `unify(&bool_ty, &discr_ty)` — same class | P3 |
| `typeck/check.rs:229,236,238` (let binding) | `unify(&place_ty, &rvalue_ty)` | ✅ Correct — place is what's expected, rvalue is what's found | — |

**New TD**: `TD-UNIFY-ARG-ORDER` (P3) — 5 more sites in typeck/check.rs
have the same expected/found swap. Not fixed in this stage because:
1. Risk of breaking existing tests that may depend on specific error messages
2. Per §12.3, can be batched in a future stage focused on error message consistency
3. Per §13.4, the fix is mechanical (arg swap) but needs comprehensive test
   impact analysis first

**Holistic plan**: TD-UNIFY-ARG-ORDER will be addressed in Stage 18.262+
(after Phase 2 lands) to avoid compounding changes.

---

## 6. §13.1 Design Alignment — §14.8 Bias Classification

Per §14.8, theoretical design vs reality implementation:

| Design Section | Bias Type | Description | Action |
|---------------|-----------|-------------|--------|
| `06-mir.md` §3.2 AggregateKind::Adt | B2 (impl > design) | Design didn't mention expected-type propagation; impl needs it for soundness | Add §3.3 "expected_ty propagation" to design doc in Stage 18.256 |
| `03-type-system.md` §3.2 tuple struct | B4 (design gray area) | Design says tuple struct ctor validates arg types; impl has soundness hole | Phase 2 fix (Stages 18.256-18.261) |
| `06-mir.md` §16.8 TD-TUPLE-CTOR-TYPECK | B1 (impl < design) | Design intends full type safety; impl has known hole | Per Stage 18.233 audit + this stage's Phase 2 plan |

---

## 7. Validation Results

### 7.1 Full Validation Pipeline (per §3.2)

| Step | Command | Result |
|------|---------|--------|
| 1 | `cargo clean` | ✅ |
| 2 | `cargo build --features llvm-backend` | ✅ 0 warnings |
| 3 | `cargo check --features llvm-backend` | ✅ 0 errors, 0 warnings |
| 4 | `cargo fmt --check` | ✅ 0 diff |
| 5 | `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ 0 warnings |
| 6 | `cargo test --features llvm-backend` | ✅ 3811 tests, 0 failures |

### 7.2 Test Count Delta

| Category | Before (18.254) | After (18.255) | Delta |
|----------|-----------------|----------------|-------|
| Lib tests | 675 | 675 | 0 |
| Integration tests | 3123 | 3136 | +13 |
| **Total** | **3798** | **3811** | **+13** |

(10 from stage18_255 regression + 3 from previously hidden submodules surfacing)

---

## 8. §14.5 Deep Review (D1-D8)

This stage is **L2** (Phase 1 = ~2 LOC fix; Phase 2 = design only, no code).
Per §1.2.1 L2 path: §3.2 + §8 + §10 + §7.3 gate review + §13.1 design
alignment. §14.5 deep review **not required** for L2.

Per §14.5.2, this stage triggers none of the §14.5 mandatory conditions:
- Not a major stage end (Stage 18.255 is mid-stage)
- Not 3 consecutive convergent rounds (still mid-v0.3)
- Not user-explicit deep review request

**However**, per §17.6 defect integration + §14.8 design writeback, the
Phase 2 plan in §4.2 above serves as the design writeback for this TD.

---

## 9. Committee Voting (per §6.3)

| Role | Vote | Reason |
|------|------|--------|
| ARCH-A | APPROVED | Phase 1 is correct minimal fix; Phase 2 plan is architecturally sound (J1-J6 pass) |
| DEV-A | APPROVED | Implementation is mechanical, low risk |
| QA-A | APPROVED | 10 regression tests verify behavior, including 1 deferred-MVP marker |
| ALG-C | APPROVED | Type system semantics preserved (Param still unifies universally; expected_ty only affects field resolution) |
| SKL-A | APPROVED | Phase 1 + design doc — no tooling concerns |

**Result: 5/5 APPROVED** (weighted: 5.5/5.5, 100%)

---

## 10. Action Plan

| # | Action | Stage | Owner | Status |
|---|--------|-------|-------|--------|
| 1 | Phase 1: unify arg order fix (Adt field + Array elem) | 18.255 | ARCH-A | ✅ Done |
| 2 | Regression tests for Phase 1 + Phase 2 deferred marker | 18.255 | QA-A | ✅ Done |
| 3 | Phase 2 design doc (this file §4) | 18.255 | ARCH-A | ✅ Done |
| 4 | TD-TUPLE-CTOR-TYPECK Phase 1 mark as resolved in tech-debt-register | 18.255 | REC-A | ✅ Done |
| 5 | TD-UNIFY-ARG-ORDER new TD registered | 18.255 | REC-A | ✅ Done |
| 6 | Phase 2a: add expected_ty param to lower_expr_* (additive) | 18.256 | ARCH-A | 🔧 Next stage |
| 7 | Phase 2b-2f: thread expected_ty through pipeline | 18.257-18.261 | ARCH-A | 🔧 Future |
| 8 | Phase 2 done: convert deferred-MVP test to assert has_errors | 18.261 | QA-A | 🔧 Future |
| 9 | TD-UNIFY-ARG-ORDER batch fix (5 sites in typeck/check.rs) | 18.262+ | ARCH-A | 🔧 Future |

---

## 11. References

- Stage 18.233 audit (TD-TUPLE-CTOR-TYPECK identification): `docs/develop/v0/stage-18/stage-18.93-deep-audit-v4-and-polish.md`
- Stage 16.53 (substitute infrastructure): `docs/develop/v0/task-11-monomorphization-design.md`
- Process doc §17 task planning: `docs/stage-committee-process.md` §17
- Process doc §13.4 six judgments: `docs/stage-committee-process.md` §13.4
- Process doc §14.8 design writeback: `docs/stage-committee-process.md` §14.8
- Regression tests: `tests/v0/stage18/plan/stage18_255_td_tuple_ctor_typeck_regression_tests.rs`
