# Stage 2.x Phase Gate Review Report (Round 2)

> **Date**: 2026-07-19
> **Reviewer**: Independent Phase Gate Audit (per §9.3 of process v3.0)
> **Verdict**: ❌ **NEEDS REVISION** — 5 P0 + 6 P1 blockers found
> **Action**: Trigger targeted fix round (Stage 2.4e), then re-audit

---

## Executive Summary

A second deep-and-broad audit of Stage 2.x was conducted per §9.3 of the
Stage Committee Process v3.0. The audit used a comprehensive test harness
(16 cases covering type errors, borrow violations, move violations, and
positive cases) and revealed **5 P0 + 6 P1 blockers** that the existing
625-test suite missed.

**Root cause**: The existing tests verified each sub-stage's internal
correctness but did not test enough **negative cases** (programs that
*should* fail). Of 13 "should error" cases, **9 were silently accepted**.

**Critical finding (G1)**: A HirId mismatch between `lower_block` (which
keys the local_map by `HirLocal.hir_id`) and `resolve::collect_pat_bindings`
(which keys the scope by `HirPat.hir_id`) causes *every* let-bound
variable's Path reference to fall back to a Ty::Error placeholder. This
single bug is responsible for ~6 of the 9 missed negative cases.

---

## Methodology

Per §9.3, the audit:
1. Ran all 625 existing tests (all pass).
2. Ran the audit example (13/15 programs clean).
3. Ran a comprehensive negative-case test harness (16 cases).
4. Manually inspected MIR output for failed cases.
5. Cross-checked source code for placeholder/stub/bug patterns.
6. Verified §9.2 ("isolated correct" 5 questions) and §9.1 (integration
   test requirements).

---

## P0 Blockers (5 — must fix before Stage 3)

### G1: HirId mismatch breaks local variable resolution
- **Location**: `src/mir/lower/mod.rs:1141` (`lower_block`, `cx.new_local(local.hir_id, ...)`) vs `src/resolve/resolver.rs:390` (`scopes.insert(ident.name, pat.hir_id)`)
- **Symptom**: `let s = "hi"; 1 - s;` silently type-checks (should error: Int - Str)
- **Root cause**: `HirLocal.hir_id` (used by MIR lower) ≠ `HirLocal.pat.hir_id` (used by resolver). The local_map is keyed by the wrong HirId, so all Path expressions referring to let-bound variables fall through to the `Ty::Error` placeholder branch.
- **Impact**: 6+ missed negative cases (use-after-move, double-mut-borrow, assign-to-borrowed, move-borrowed, etc.). The compiler silently accepts many invalid programs.
- **Fix**: In `lower_block`, change `cx.new_local(local.hir_id, ...)` to `cx.new_local(local.pat.hir_id, ...)`.

### G2: NLL kills borrows at RHS last-use, before LHS write check
- **Location**: `src/borrowck/mod.rs:73-91` (check_mir_body + kill_expired_borrows)
- **Symptom**: `let r = &x; x = x + *r;` silently compiles (should error: assign to borrowed)
- **Root cause**: `kill_expired_borrows` runs *after* `check_statement`, but `check_statement` calls `check_rvalue` (which reads RHS operands, including `*r` — the last use of `r`) *before* `check_place_write` (which checks if the LHS is borrowed). The last-use of `r` triggers a kill, so by the time `check_place_write` runs, the borrow is already gone.
- **Impact**: Any code that reads a borrow in the same statement that writes the borrowed place is silently accepted.
- **Fix**: Move `kill_expired_borrows` to run *after* the full statement (both rvalue and place_write checks). Or: don't kill borrows within the same statement that created the last-use read.

### G3: Call type checking doesn't verify arg count or fn signature
- **Location**: `src/typeck/checker.rs:171-202` (check_terminator for Call)
- **Symptom**: `add(1)` where `fn add(a: i32, b: i32) -> i32` silently compiles
- **Root cause**: The Call type checker only unifies when `func_ty` is `FnPtr(sig)`. But function definitions produce `FnDef(def_id, [])` — there's no mechanism to look up the fn signature from the DefId. The FnDef case falls through to "no constraint checked".
- **Impact**: Wrong arg counts, wrong arg types, and wrong return types for function calls are all silently accepted.
- **Fix**: Add a `FnSigMap` (DefId → Sig) populated during HIR traversal. In Call type checking, look up the sig from DefId and unify args + return.

### G4: Undefined function calls silently accepted
- **Location**: `src/resolve/resolver.rs` + `src/mir/lower/mod.rs:530-536` (Path)
- **Symptom**: `undefined_fn()` compiles without error
- **Root cause**: When `path.res` is `Res::Unknown` or `Res::Err`, MIR lower falls through to the `Ty::Error` placeholder branch. Typeck treats `Error` as "always succeeds" (intentional error recovery). So the undefined name is silently swallowed.
- **Impact**: Typos in function names go undetected.
- **Fix**: Either (a) emit a resolve error for `Res::Unknown` paths, or (b) in MIR lower, emit a typeck error for `Res::Unknown` paths.

### G5: No mutability tracking — immutable variables can be reassigned
- **Location**: `src/borrowck/mod.rs` (no mutability field on LocalDecl)
- **Symptom**: `let x = 1; x = 2;` silently compiles
- **Root cause**: The borrow checker has no concept of variable mutability. `LocalDecl` has no `mut: bool` field. The HIR captures `let mut x` vs `let x` in the pattern's `BindingMode`, but this information is lost during MIR lowering.
- **Impact**: The compiler accepts Rust code that would be rejected (`let x = 1; x = 2;` is a hard error in Rust).
- **Fix**: Add `mutability: Mutability` to `LocalDecl`. In `lower_block`, set it from the pattern's `BindingMode`. In `check_place_write`, reject writes to immutable locals (unless the write is the initial `let` binding itself).

---

## P1 Issues (6 — should fix before Stage 3)

### G6: Use-after-move on non-Copy types silently accepted (G1 side-effect)
- **Location**: `src/borrowck/mod.rs:217-244` (check_operand for Copy)
- **Symptom**: `let s = "hi"; let t = s; let u = s;` silently compiles (Str is not Copy)
- **Root cause**: G1's HirId mismatch causes the second `s` to be a Ty::Error placeholder. `ty_is_copy(Error)` returns `true` (intentional). After G1 is fixed, this case should work — but verify with regression tests.

### G7: MethodCall uses Error placeholder as func
- **Location**: `src/mir/lower/mod.rs:1019-1032`
- **Symptom**: `receiver.method(args)` produces a Call with `func: Const(Ty::Error)`. The method is never resolved.
- **Impact**: Any method call silently type-checks. Codegen would crash.
- **Fix**: Stage 3 (requires TraitResolver).

### G8: Repeat array `[val; N]` simplified to 1-element
- **Location**: `src/mir/lower/mod.rs:946-961`
- **Symptom**: `[0; 5]` produces a 1-element array, not 5.
- **Impact**: Wrong array lengths at codegen.
- **Fix**: Stage 3 (requires const-eval for `N`).

### G9: Struct literal uses AggregateKind::Tuple
- **Location**: `src/mir/lower/mod.rs:963-983`
- **Symptom**: `Point { x: 1, y: 2 }` produces a Tuple aggregate, losing the Adt DefId and field names.
- **Impact**: Field access on struct literals won't work correctly.
- **Fix**: Stage 3 (requires Adt definition lookup).

### G10: Public `check_crate` APIs use stale lowering path
- **Location**: `src/typeck/checker.rs:424` + `src/borrowck/mod.rs:661`
- **Symptom**: `typeck::check_crate(hir, interner)` doesn't pass return_ty or unify table — produces different results than the driver.
- **Impact**: Any consumer that calls `check_crate` directly (instead of `driver::compile`) gets wrong type inference.
- **Fix**: Update `check_crate` to use `lower_hir_body_to_mir_full` + `TypeChecker::with_unify`, matching the driver.

### G11: All references share `Region::Var(RegionVid(0))`
- **Location**: `src/mir/lower/mod.rs:477`
- **Symptom**: Every `&T` and `&mut T` uses the same region variable, so the borrow checker can't distinguish lifetimes.
- **Impact**: NLL is partially sound (uses PlacePath + last-use map instead), but full region inference is impossible.
- **Fix**: Stage 3 (region inference).

---

## P2/P3 Issues

### G12 (P2): `Rvalue::BinaryOp2` is dead code
- **Location**: `src/mir/lower/mod.rs:899-913` (Range uses Aggregate) vs `src/mir/lvalue.rs:87` (BinaryOp2 variant defined)
- **Impact**: Dead code; `BinaryOp2` is never constructed.
- **Fix**: Remove `BinaryOp2` variant, or use it for Range lowering.

### G13 (P2): No `clear_borrows_on_local` call at StorageDead
- **Location**: `src/borrowck/mod.rs`
- **Impact**: Borrows on a local are not killed when StorageDead is emitted. Currently relies on NLL last-use map, which may not cover all cases.
- **Fix**: Hook into StorageDead statements to clear borrows on the dead local.

### G14 (P3): Audit count off by 1
- **Location**: `docs/stage-2.4d-gate-review.md`
- **Symptom**: Document says "14/15 programs compile cleanly" but actual is "13/15".
- **Fix**: Update doc.

---

## §9.2 "Isolated Correct" Defense — 5 Questions

| # | Question | Answer |
|---|----------|--------|
| Q1 | Does output contain placeholder/stub? | **YES** — Repeat, Struct literal, MethodCall, MacroCall all use Ty::Error placeholders (P1 G7-G9). Region::Var(0) shared (P1 G11). |
| Q2 | Can next stage consume this output? | **PARTIAL** — MIR is structurally valid, but G1 (HirId mismatch) means local variable references produce wrong MIR. Codegen would produce incorrect code. |
| Q3 | End-to-end test coverage? | **PARTIAL** — 58 integration tests, but mostly positive cases. Negative case coverage is weak (9 of 13 missed). |
| Q4 | P3 tech debt affecting next stage? | **YES** — G7 (MethodCall), G8 (Repeat), G9 (Struct) all affect codegen correctness. Should be P1, not P3. |
| Q5 | `check_crate` actually called? | **YES** by driver; **NO** by external consumers (G10). The driver uses the new path; the public `check_crate` API uses the old path. |

**Verdict**: Q1, Q2, Q4, Q5 all fail. Per §4.1, the P3 items (G7-G9) must be upgraded to P1.

---

## §9.1 Integration Test Requirements

| Requirement | Status |
|-------------|--------|
| ≥1 positive integration test | ✅ 13 programs compile cleanly |
| ≥1 negative integration test | ❌ Only 4 of 13 negative cases detected (G1-G5 responsible) |
| ≥1 cross-stage consumption test | ✅ TypeckResults populated, StorageLive emitted |

**Verdict**: Negative integration test coverage is insufficient. The audit harness used in this review should be added as a permanent test suite.

---

## Committee Vote (5 roles)

| Role | Weight | Vote | Reason |
|------|--------|------|--------|
| Compiler Engineer (Architect) | 2.0 | **NEEDS REVISION** | G1 is a fatal soundness bug — local variable resolution is broken. G3 (Call type checking) is incomplete. G5 (mutability) is a missing core feature. Cannot enter Stage 3 with these. |
| Soundness Reviewer | 1.5 | **NEEDS REVISION** | G1, G2, G5 are soundness holes. The compiler silently accepts invalid programs. This violates the core guarantee of a type-safe language. |
| Testing & QA Lead | 1.0 | **NEEDS REVISION** | Existing tests missed 9/13 negative cases. Test coverage is unbalanced toward positive cases. Need negative-case test suite before approval. |
| Type System Theorist | 1.0 | **NEEDS REVISION** | G3 (no fn sig lookup), G11 (single Region var) mean the type system is not actually enforcing what it claims. Type soundness is compromised. |
| Tooling & DX Lead | 1.0 | **NEEDS REVISION** | G10 (public API inconsistency) breaks any external consumer. G14 (doc error). |

**Weighted total**: 0.0 / 5.5 = **0% approval** (need ≥95%)

**Unanimous NEEDS REVISION.** Per §5.2, trigger second inner loop (Stage 2.4e).

---

## Targeted Fix Plan: Stage 2.4e

### Stage 2.4e-1: G1 — HirId mismatch (P0)
- One-line fix in `lower_block`: `local.hir_id` → `local.pat.hir_id`
- Add regression tests for all 9 missed negative cases

### Stage 2.4e-2: G2 — NLL kill timing (P0)
- Move `kill_expired_borrows` to run *after* `check_statement` completes
  (currently it runs between check_rvalue and check_place_write)

### Stage 2.4e-3: G3 — Call type checking (P0)
- Add `FnSigMap` (DefId → Sig) populated from HIR
- In Call type checking, look up sig from DefId and unify args + return

### Stage 2.4e-4: G4 — Undefined function detection (P0)
- In MIR lower or typeck, emit error for `Res::Unknown` paths

### Stage 2.4e-5: G5 — Mutability tracking (P0)
- Add `mutability` field to `LocalDecl`
- In `check_place_write`, reject writes to immutable locals

### Stage 2.4e-6: G6-G11 — P1 fixes
- G6: regression tests for use-after-move (after G1 fixed)
- G10: update public `check_crate` APIs to match driver
- G7-G9, G11: deferred to Stage 3 (require TraitResolver, const-eval, region inference)

### Stage 2.4e-7: Negative-case test suite
- Add `tests/v0/stage2/plan/negative_cases_tests.rs` with all 13 negative cases from this audit

---

## Re-audit Plan

After Stage 2.4e completes:
1. Re-run all 625 + new negative-case tests
2. Re-run audit example
3. Re-run this 5-role committee vote
4. If unanimous APPROVED, enter Stage 3

**Estimated effort**: 2-4 hours of focused work.
