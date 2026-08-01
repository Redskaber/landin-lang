# Stage 15.37 — Driver Switch (DEFERRED) + Legacy Deprecation + GAP-1 Conflict

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.162.0 → v0.163.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)
> **v0.2 Phase 2 Task 7 (step 3 of 4 — DEFERRED)**: Activate fixpoint dataflow NLL (HP-10)
> **Design doc**: `docs/lang-design/23-nll-fixpoint.md`
> **Prior stage**: `docs/develop/v0/stage-15/stage-15.36-kill-expired-borrows-dataflow.md`

## 1. Executive Summary

Stage 15.37 was planned as the final step of the 4-step NLL fixpoint
migration (Stage 15.34-15.37): switch the driver to use
`check_mir_body_with_dataflow`, deprecate the legacy `check_mir_body`,
and remove `compute_last_use_map` + `kill_expired_borrows` (dead code).

**The driver switch was attempted and reverted.** Stage 15.37 discovered
a **semantic conflict** between the dataflow path (algorithmically
correct NLL) and the project's Stage 14.81 GAP-1 soundness fix. The
dataflow path accepts `let r1 = &mut x; let r2 = &mut x;` (correct NLL
— `r1` is dead after `r2` is created, so its borrow expires), but 112
conformance tests depend on the GAP-1 stricter semantics (the legacy
path rejects this pattern).

**Decision**: Defer the driver switch to a future stage that reconciles
the GAP-1 decision with NLL correctness. The legacy `check_mir_body`
remains the driver's active path but is now marked `#[deprecated]` to
signal that the dataflow path is the long-term direction. The dataflow
path (`check_mir_body_with_dataflow`) remains available for testing and
future migration.

Per §1.0 原則 1 "长期 > 短期": the dataflow path is the correct
long-term design. Per §1.0 原則 5 "报错 > 静默": the GAP-1 stricter
semantics are the safer short-term choice. The two will be reconciled
in a future stage.

## 2. What Was Done

### 2.1 Legacy `check_mir_body` marked `#[deprecated]`

Both the method (`BorrowChecker::check_mir_body`) and the free function
(`borrowck::check_mir_body`) are now marked `#[deprecated]` with a note
pointing to `check_mir_body_with_dataflow`:

```rust
#[deprecated(
    note = "Use `check_mir_body_with_dataflow` (v0.2 sound dataflow analysis) instead — \
            this legacy path is unsound for loops and conditionals. Will be removed in v0.3."
)]
#[allow(deprecated)]  // needed because the free fn calls the deprecated method internally
pub fn check_mir_body(mir: &MirBody) -> Vec<BorrowError> { ... }
```

Per §23.1 rule 6: deprecated entry points must have a `note = "..."`
pointing to the sounder alternative. The `#[allow(deprecated)]` on the
free function is needed because it internally calls the deprecated
method (this is the standard pattern — see `typeck::mod.rs` which uses
the same `#[allow(deprecated)]` on its re-exports).

### 2.2 Driver switch attempted, then reverted

The driver was initially changed to call
`bc.check_mir_body_with_dataflow(&mir)` instead of `bc.check_mir_body(&mir)`.
After running the conformance suite, **112 tests failed** — all cases
where the dataflow path accepts a program that the legacy path rejects
(the GAP-1 soundness patterns).

The driver was reverted to use `bc.check_mir_body(&mir)` (wrapped in
`#[allow(deprecated)]` to suppress the deprecation warning at the call
site). The driver's behavior is now identical to v0.162.0.

### 2.3 `check_crate` reverted to use legacy path

The `check_crate` free function (already `#[deprecated]` from Stage 3.63)
was briefly changed to use `check_mir_body_with_dataflow` internally,
then reverted to use `check_mir_body` (matching the driver's behavior).
Added `#[allow(deprecated)]` and updated the doc comment to explain the
deferral.

### 2.4 Test files patched with `#[allow(deprecated)]`

7 existing test files intentionally call the legacy `check_mir_body` to
test the legacy path. These files were patched with
`#![allow(deprecated)]` to suppress the expected deprecation warnings:

- `tests/v0/stage7/plan/design_writeback_verification_tests.rs`
- `tests/v0/stage7/plan/deep_review_tests.rs`
- `tests/v0/stage7/plan/systematic_review_v014_tests.rs`
- `tests/v0/stage7/plan/region_inference_tests.rs`
- `tests/v0/stage8/plan/lifetime_elision_tests.rs`
- `tests/v0/stage8/plan/deep_review_tests.rs`
- `tests/v0/stage8/plan/drop_elaboration_tests.rs`

The Stage 15.36 test file (`kill_borrows_dataflow_tests.rs`) was also
patched — it intentionally calls both paths for parity comparison.

The Stage 15.37 test file (`stage15_37_driver_switch_tests.rs`) was
created with `#![allow(deprecated)]` from the start.

### 2.5 `compute_last_use_map` + `kill_expired_borrows` RETAINED

The original plan was to remove `compute_last_use_map` and
`kill_expired_borrows` as dead code after the driver switch. Since the
driver switch was deferred, these functions are NOT dead code — they're
still used by the legacy `check_mir_body` path. They remain as-is,
documented as "retained until the driver switch is completed in a
future stage".

## 3. Why the Driver Switch Was Deferred

### 3.1 The GAP-1 soundness fix (Stage 14.81)

Stage 14.81 fixed **GAP-1 (NLL soundness regression)**: the long-standing
silent acceptance of unsound borrow patterns like
`let r1 = &mut x; let r2 = &mut x;`. The fix was a 1-line change in
`check_rvalue` to handle `Operand::Copy` of ref temps (in addition to
`Operand::Move`), ensuring the borrow's `ref_local` is correctly
transferred from the temporary to the user-visible local.

After the fix, 113 conformance tests were flipped from `compile_ok`
back to `compile_error` (the correct behavior — these are unsound
programs). The fix relies on the legacy NLL algorithm's behavior: a
borrow's `ref_local` that's never read stays in the borrow set forever
(because `compute_last_use_map` only records reads, and a never-read
local never has its "last use" recorded, so `kill_expired_borrows`
never kills it).

### 3.2 The dataflow path's correct NLL behavior

The dataflow path (`check_mir_body_with_dataflow`) uses fixpoint
liveness analysis. For `let r1 = &mut x; let r2 = &mut x;`:

1. `r1` is assigned via `tmp1 = &mut x; r1 = Copy(tmp1);`.
2. After the transfer, the borrow's `ref_local = r1`.
3. `r1` is never read after assignment.
4. The dataflow liveness analysis correctly identifies `r1` as dead
   (it's not in any `LiveIn` or `LiveOut` set after its assignment).
5. `kill_expired_borrows_dataflow` kills `r1`'s borrow (because `r1`
   is not live after the assignment).
6. `tmp2 = &mut x; r2 = Copy(tmp2);` creates a second borrow on `x` —
   no conflict, because the first borrow was killed.

This is **correct NLL semantics** — it's what real Rust does. `r1` is
dead, so its borrow expires, so `r2 = &mut x` is allowed.

### 3.3 The conflict

The project's GAP-1 decision (Stage 14.81) is that
`let r1 = &mut x; let r2 = &mut x;` should be a `compile_error` —
**even when `r1` is never used after `r2` is created**. This is stricter
than real Rust NLL. The 112 conformance tests that depend on this
behavior encode this stricter semantics.

The dataflow path is algorithmically correct but violates the project's
GAP-1 decision. Switching the driver would regress 112 conformance
tests.

### 3.4 The decision

Per §1.0 原則 1 "长期 > 短期": the dataflow path is the correct
long-term design. Per §1.0 原則 5 "报错 > 静默": the GAP-1 stricter
semantics are the safer short-term choice.

**The driver switch is deferred** until a future stage reconciles the
GAP-1 decision with NLL correctness. The reconciliation requires a
design decision about which semantics the project wants:

- **Option A (adopt real Rust NLL)**: Flip the 112 conformance tests
  back to `compile_ok` (they were `compile_error` from Stage 14.81).
  Switch the driver to the dataflow path. This is correct NLL but
  relaxes the soundness guarantee.
- **Option B (keep GAP-1 stricter semantics)**: Modify the dataflow
  path to NOT kill borrows whose `ref_local` is dead-but-in-scope.
  This would require a different liveness query (e.g., "is `ref_local`
  in scope after this point?" rather than "is `ref_local` live after
  this point?"). This preserves GAP-1 but is not standard NLL.
- **Option C (hybrid)**: Use the dataflow path for loops/conditionals
  (where it's strictly better) but fall back to the legacy path for
  straight-line borrow patterns (where GAP-1 wants the stricter
  behavior). This is complex but might be the best of both worlds.

This decision is deferred to a future stage (likely v0.3 when the
project revisits the overall NLL design).

## 4. The GAP-1 Semantic Conflict (full analysis)

### 4.1 The pattern

```rust
fn main() {
    let mut x = 1;
    let r1 = &mut x;  // borrow created, ref_local = r1
    let r2 = &mut x;  // GAP-1: should conflict; NLL: r1 is dead, no conflict
}
```

### 4.2 Legacy path behavior (current driver)

MIR lowering produces:
```
bb0:
  stmt 0: tmp1 = &mut x        (borrow created, ref_local = tmp1)
  stmt 1: r1 = Copy(tmp1)      (transfer_borrow_ref: tmp1 → r1)
  stmt 2: tmp2 = &mut x        (second borrow — should conflict with r1's)
  stmt 3: r2 = Copy(tmp2)      (transfer_borrow_ref: tmp2 → r2)
  term:   return
```

`compute_last_use_map` records:
- `tmp1` last read at (bb0, 1) — the `r1 = Copy(tmp1)` statement.
- `r1` never read → not in the map.

Walk:
- `stmt_idx=0`: process `tmp1 = &mut x` (creates borrow, ref_local = tmp1).
- `stmt_idx=1`: kill at (bb0, 0) — no local has last_use == (bb0, 0).
  Process `r1 = Copy(tmp1)` — transfers borrow ref_local tmp1 → r1.
- `stmt_idx=2`: kill at (bb0, 1) — `tmp1` has last_use == (bb0, 1).
  `kill_borrows_of_local(tmp1)` — but the borrow now has ref_local = r1
  (after the transfer), so this kills nothing. The borrow (ref_local = r1)
  stays alive.
  Process `tmp2 = &mut x` — tries to add a second borrow on `x` →
  **conflict detected** (r1's borrow is still active).

Result: **compile_error** (GAP-1 soundness).

### 4.3 Dataflow path behavior

`compute_liveness` produces:
- `LiveOut[bb0]` = ∅ (no successors read anything).
- `LiveIn[bb0]` = ∅ (no reads in bb0 except tmp1, tmp2 which are killed
  by their defs).

Actually, let me trace more carefully. `r1 = Copy(tmp1)` reads `tmp1`,
so `tmp1 ∈ Use[bb0]`. `r2 = Copy(tmp2)` reads `tmp2`, so `tmp2 ∈ Use[bb0]`.
`r1` and `r2` are never read, so they're not in `Use[bb0]`.

- `LiveOut[bb0]` = ∅.
- `LiveIn[bb0]` = `Use[bb0] ∪ (LiveOut[bb0] - Def[bb0])` = `{tmp1, tmp2} ∪ ∅` = `{tmp1, tmp2}`.

Walk (using `kill_expired_borrows_dataflow`):
- `stmt_idx=0`: process `tmp1 = &mut x` (creates borrow, ref_local = tmp1).
- `stmt_idx=1`: kill at (bb0, 0). `compute_live_after_point(mir, live_out, bb0, 0)`:
  - Start with `LiveOut[bb0]` = ∅.
  - Fold terminator (Return: no reads/writes) → ∅.
  - Fold stmt 3 (`r2 = Copy(tmp2)`): reads `tmp2`, writes `r2` → live = {tmp2}.
  - Fold stmt 2 (`tmp2 = &mut x`): writes `tmp2`, no reads → live = {}.
  - Fold stmt 1 (`r1 = Copy(tmp1)`): reads `tmp1`, writes `r1` → live = {tmp1}.
  - Result: {tmp1}.
  - `tmp1` is in the live set → borrow with ref_local = tmp1 is NOT killed.
  - But wait — the borrow's ref_local was tmp1 at this point (the transfer
    happens DURING stmt 1, not before). So the borrow is still ref_local = tmp1.
  - Actually, the kill happens BEFORE check_statement for stmt_idx=1. So
    at the kill point, the borrow is still ref_local = tmp1 (the transfer
    hasn't happened yet). `tmp1 ∈ live_after` → borrow NOT killed. Good.
  - Process `r1 = Copy(tmp1)` — transfers borrow ref_local tmp1 → r1.
- `stmt_idx=2`: kill at (bb0, 1). `compute_live_after_point(mir, live_out, bb0, 1)`:
  - Start with `LiveOut[bb0]` = ∅.
  - Fold terminator → ∅.
  - Fold stmt 3 (`r2 = Copy(tmp2)`): reads `tmp2` → live = {tmp2}.
  - Fold stmt 2 (`tmp2 = &mut x`): writes `tmp2` → live = {}.
  - Result: {}.
  - `r1` is NOT in the live set → borrow with ref_local = r1 IS killed.
  - `kill_borrows_of_local(r1)` — kills r1's borrow.
  - Process `tmp2 = &mut x` — tries to add a second borrow on `x` →
    **no conflict** (r1's borrow was just killed).

Result: **compile_ok** (correct NLL, but violates GAP-1).

### 4.4 The root cause

The legacy path "accidentally" preserves GAP-1 soundness because
`compute_last_use_map` only records reads, and a never-read `ref_local`
(like `r1`) never gets its "last use" recorded, so its borrow is never
killed. This is actually a **bug** in the legacy path (it should kill
the borrow when `r1` goes out of scope), but it happens to produce the
GAP-1-desired behavior.

The dataflow path "correctly" identifies `r1` as dead and kills its
borrow, which is standard NLL but violates GAP-1.

### 4.5 Reconciliation options

**Option A (adopt real Rust NLL)**:
- Flip the 112 conformance tests from `compile_error` back to `compile_ok`.
- Switch the driver to `check_mir_body_with_dataflow`.
- Pros: correct NLL, simpler code, matches Rust semantics.
- Cons: relaxes the soundness guarantee that GAP-1 established.

**Option B (keep GAP-1 stricter semantics)**:
- Modify `kill_expired_borrows_dataflow` to use "in scope" instead of
  "live" — a borrow is killed only when its `ref_local` goes out of
  scope (block exit), not when it becomes dead.
- This requires a different analysis (scope-based, not liveness-based).
- Pros: preserves GAP-1 soundness.
- Cons: not standard NLL, more complex, defeats the purpose of the
  dataflow migration for straight-line code.

**Option C (hybrid)**:
- Use the dataflow path for loops/conditionals (where it's strictly
  better — the legacy path is unsound there).
- Use the legacy path for straight-line borrow patterns (where GAP-1
  wants the stricter behavior).
- Pros: best of both worlds.
- Cons: complex to implement (need to detect "is this a loop/conditional
  context?"), two code paths to maintain.

**Recommendation**: Defer to v0.3 when the project revisits the overall
NLL design. The dataflow path is available and tested; the legacy path
remains the driver's default. The 112 conformance tests document the
GAP-1 semantics that must be preserved (or deliberately relaxed) in the
reconciliation.

## 5. What Was NOT Done

Per the original Stage 15.37 plan, the following were NOT done (deferred
to the future reconciliation stage):

- ❌ Switch the driver to `check_mir_body_with_dataflow`.
- ❌ Remove `compute_last_use_map` (still used by legacy `check_mir_body`).
- ❌ Remove `kill_expired_borrows` (still used by legacy `check_mir_body`).
- ❌ Remove the `LastUseMap` type alias (still used by `compute_last_use_map`).

These will be done in a future stage after the GAP-1 reconciliation
decision is made.

## 6. API Naming Compliance (§23)

| Symbol | Pattern | Status |
|--------|---------|--------|
| `check_mir_body` (method + free fn) | `#[deprecated]` with note pointing to `check_mir_body_with_dataflow` (§23.1 rule 6) | ✅ |
| `check_mir_body_with_dataflow` | `<verb>_<noun>_<noun>` with `_with_dataflow` suffix (Stage 15.36) | ✅ |
| `check_crate` | `#[deprecated]` with note (Stage 3.63 + Stage 15.37 doc update) | ✅ |
| `#[allow(deprecated)]` on test files | Standard pattern for tests that intentionally exercise deprecated APIs | ✅ |

Per §23.1 rule 6: "Module re-exports of deprecated items must be wrapped
in `#[allow(deprecated)]`." The same pattern is applied to test files
that intentionally call deprecated APIs.

## 7. Testing

### 7.1 New integration tests (9, in `tests/v0/stage15/plan/stage15_37_driver_switch_tests.rs`)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_37_legacy_check_mir_body_still_callable` | Deprecated `check_mir_body` free fn still works (deprecation is a warning, not removal) |
| 2 | `stage15_37_legacy_borrow_checker_method_still_callable` | Deprecated `BorrowChecker::check_mir_body` method still works |
| 3 | `stage15_37_driver_uses_legacy_path_no_regression` | Driver still uses legacy path (no behavior change from v0.162.0) |
| 4 | `stage15_37_driver_preserves_gap1_soundness` | Driver still rejects double-mut-borrow (GAP-1 fix preserved) |
| 5 | `stage15_37_dataflow_path_still_accessible` | `check_mir_body_with_dataflow` is callable on real MIR |
| 6 | `stage15_37_dataflow_path_handles_loop_borrow` | Dataflow path correctly handles loop-carried borrows |
| 7 | `stage15_37_gap1_semantic_conflict_documented` | **Documents the GAP-1 conflict**: legacy rejects, dataflow accepts |
| 8 | `stage15_37_parity_on_valid_program` | Legacy and dataflow agree on valid programs (no conflict) |
| 9 | `stage15_37_parity_on_single_borrow` | Legacy and dataflow agree on single-borrow programs |

### 7.2 Regression strategy

- All 173 lib tests pass (zero regression).
- All 2039 + 9 = 2048 integration tests pass (zero regression + 9 new).
- All 5216 conformance tests pass (zero regression — driver switch was
  reverted, so conformance behavior is identical to v0.162.0).
- 0 clippy warnings, fmt clean.

## 8. Migration Plan (Stages 15.34-15.37) — Updated Status

| Stage | Status | Description |
|-------|--------|-------------|
| 15.34 | ✅ DONE (v0.160.0) | NLL fixpoint design doc |
| 15.35 | ✅ DONE (v0.161.0) | `compute_liveness` fixpoint function |
| 15.36 | ✅ DONE (v0.162.0) | `kill_expired_borrows_dataflow` + `check_mir_body_with_dataflow` |
| **15.37** | **⚠️ PARTIAL (v0.163.0)** | **Legacy `check_mir_body` deprecated; driver switch DEFERRED due to GAP-1 conflict** |
| future | ⏳ DEFERRED | Reconcile GAP-1 with NLL correctness, then switch driver + remove legacy code |

## 9. Verification

- `cargo build --features llvm-backend` — ✅ clean build, 0 warnings
- `cargo test --features llvm-backend --test all_tests stage15_37_driver_switch_tests` — ✅ 9/9 PASS
- All 173 lib tests pass (zero regression)
- All 2048 integration tests pass (2039 + 9 new, zero regression)
- All 5216 conformance tests pass (zero regression — driver switch reverted)
- 0 clippy warnings, fmt clean

## 10. Stage Gate Review (self-review per §9.3)

| Check | Status |
|-------|--------|
| Design doc exists (`docs/lang-design/23-nll-fixpoint.md`) | ✅ |
| Implementation matches design (with documented deferral) | ✅ |
| Unit + integration tests cover the new behavior | ✅ 9 new tests |
| GAP-1 semantic conflict documented with regression test | ✅ |
| API naming compliance (§23) — deprecation notes point to alternative | ✅ |
| §16 interface isolation | ✅ — no new cross-stage coupling |
| §15 最优 > 最小 — deferral is the right trade-off (not a hack) | ✅ |
| 0 clippy warnings | ✅ |
| fmt clean | ✅ |
| Zero regression on existing tests (including 5216 conformance) | ✅ |

## 11. Conclusion

Stage 15.37 is a **partial completion** of the 4-step NLL fixpoint
migration. The legacy `check_mir_body` is now deprecated (signaling
the long-term direction), but the driver switch is deferred due to a
discovered semantic conflict with the Stage 14.81 GAP-1 soundness fix.

The dataflow path (`check_mir_body_with_dataflow`) is algorithmically
correct NLL and remains available for testing and future migration.
The conflict is documented with a regression test
(`stage15_37_gap1_semantic_conflict_documented`) so future reconciliation
work has a clear acceptance criterion.

The migration will be completed in a future stage after the project
makes a design decision about GAP-1 vs. NLL correctness (Options A/B/C
in §4.5). This is deferred to v0.3 when the overall NLL design is
revisited.

**Key takeaway**: The dataflow migration (Stages 15.34-15.36) was
technically successful — the algorithm is correct, the API is clean,
and the tests pass. Stage 15.37 discovered that the migration has a
**semantic** implication (GAP-1 stricter semantics vs. NLL correctness)
that requires a project-level design decision, not just a code change.
Documenting this and deferring is the right call per §1.0 原則 1
"长期 > 短期" — rushing the switch would regress 112 conformance tests.
