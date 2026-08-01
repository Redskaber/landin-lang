# Stage 15.36 — `kill_expired_borrows_dataflow` (HP-10 step 2 of 4)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.161.0 → v0.162.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)
> **v0.2 Phase 2 Task 7 (step 2 of 4)**: Activate fixpoint dataflow NLL (HP-10)
> **Design doc**: `docs/lang-design/23-nll-fixpoint.md`
> **Prior stage**: `docs/develop/v0/stage-15/stage-15.35-nll-fixpoint-liveness.md`

## 1. Executive Summary

Stage 15.36 wires the fixpoint liveness analysis from Stage 15.35 into a
**new borrow-checker entry point** `check_mir_body_with_dataflow`. The new
entry point uses `kill_expired_borrows_dataflow` — a method that consults
the fixpoint `LiveOutMap` instead of the legacy single-pass `LastUseMap` —
to expire borrows. This is the **second of four migration steps**
(Stage 15.34-15.37) to replace the unsound legacy borrow-check path with a
proper dataflow analysis.

The legacy entry point (`check_mir_body`) is **retained unchanged**. Both
paths coexist for one stage so we can A/B-test the dataflow path on real
MIR before flipping the switch in Stage 15.37. The driver still uses the
legacy path; Stage 15.37 will switch the driver to the dataflow path and
remove `compute_last_use_map`.

Per §1.0 原則 1 "长期 > 短期": keeping both paths for one stage is the
right trade-off — it lets us validate the dataflow path on real code
before committing. Per §1.0 原則 3 "显式 > 隐式": the choice of analysis
is explicit in the method name (`_with_dataflow` suffix), not a hidden
flag.

## 2. Why This Stage?

### 2.1 The problem with the legacy `kill_expired_borrows`

The Stage 6.14 `kill_expired_borrows` consults the legacy
`compute_last_use_map`, which is unsound for:

1. **Loops**: a local's "last use" inside a loop body is not its true
   last use — the next iteration will read it again. The legacy approach
   kills borrows too early, producing false-positive borrow errors on
   the second iteration.
2. **Conditionals**: a borrow alive in one branch may still be live in
   the other branch. The legacy approach doesn't track branch liveness.

### 2.2 The dataflow-driven solution

`kill_expired_borrows_dataflow` consults the fixpoint `LiveOutMap`
(computed by `compute_liveness` in Stage 15.35). For each program point
`(bb_id, stmt_idx)`, it computes "the set of locals live immediately
after this point" and kills any active borrow whose `ref_local` is NOT
in that set.

The "live after point" computation is performed by a new helper
`compute_live_after_point`, which back-propagates from `LiveOut[bb_id]`
through the remaining statements in the block (and the terminator),
applying the standard liveness transfer function `live = Use ∪ (live - Def)`
at each step.

### 2.3 Why this is the right time

Stage 15.35 landed the fixpoint `compute_liveness` algorithm. Stage
15.36 builds on that foundation by adding the per-statement liveness
query (`compute_live_after_point`) and the dataflow-driven kill path
(`kill_expired_borrows_dataflow`). The migration is staged so each step
is independently testable — Stage 15.36 adds the API without wiring it
into the driver, and Stage 15.37 will flip the switch.

## 3. Design

### 3.1 New public API

```rust
// src/borrowck/mod.rs (BorrowChecker impl)

/// Dataflow-driven borrow check entry point. Uses fixpoint liveness
/// instead of the legacy single-pass last-use map.
pub fn check_mir_body_with_dataflow(&mut self, mir: &MirBody);

/// Free-function wrapper (mirrors the relationship between
/// `check_mir_body` and `BorrowChecker::check_mir_body`).
pub fn check_mir_body_with_dataflow(mir: &MirBody) -> Vec<BorrowError>;

// src/borrowck/liveness.rs

/// Compute the set of locals live immediately after (bb_id, stmt_idx).
/// Back-propagates from LiveOut[bb_id] through the remaining statements
/// + terminator, applying the liveness transfer function.
pub fn compute_live_after_point(
    mir: &MirBody,
    live_out: &LiveOutMap,
    bb_id: BasicBlockId,
    stmt_idx: usize,
) -> HashSet<LocalId>;

// src/borrowck/borrow_set.rs (BorrowSet impl)

/// Iterate over the distinct ref_locals of all active borrows.
/// Used by kill_expired_borrows_dataflow to find which borrows to kill.
pub fn active_ref_locals(&self) -> impl Iterator<Item = LocalId> + '_;
```

### 3.2 The `kill_expired_borrows_dataflow` algorithm

```text
kill_expired_borrows_dataflow(mir, live_out, bb_id, stmt_idx):
    live_after = compute_live_after_point(mir, live_out, bb_id, stmt_idx)

    # Kill any active borrow whose ref_local is NOT in the live set.
    locals_to_kill = self.borrows.active_ref_locals()
                              .filter(|l| !live_after.contains(l))
                              .collect()
    for local in locals_to_kill:
        self.borrows.kill_borrows_of_local(local)
```

### 3.3 The `compute_live_after_point` algorithm

```text
compute_live_after_point(mir, live_out, bb_id, stmt_idx):
    bb = mir.basic_blocks[bb_id]
    stmt_count = bb.statements.len()
    term_idx = stmt_count

    # Start with LiveOut[bb_id].
    live = live_out.get(bb_id).unwrap_or(∅)

    # If the program point is at-or-after the terminator, LiveOut is
    # already the answer.
    if stmt_idx >= term_idx:
        return live

    # Fold in the terminator's Use/Def.
    fold_use_def(live, terminator_reads(bb.terminator),
                       terminator_writes(bb.terminator))

    # Walk backwards over statements after stmt_idx.
    for s in (stmt_idx + 1 .. stmt_count).rev():
        fold_use_def(live, statement_reads(bb.statements[s]),
                           statement_writes(bb.statements[s]))

    return live

# Helper: apply liveness transfer function in place.
fold_use_def(live, uses, defs):
    for d in defs: live.remove(d)
    for u in uses: live.insert(u)
```

### 3.4 Walk structure parity with `check_mir_body`

`check_mir_body_with_dataflow` performs the **exact same forward walk**
as `check_mir_body`. The only difference is the kill path:

| Step | `check_mir_body` (legacy) | `check_mir_body_with_dataflow` (new) |
|------|---------------------------|--------------------------------------|
| Pre-pass | `compute_last_use_map(mir)` | `compute_liveness(mir)` → `(_live_in, live_out)` |
| Per-statement kill | `kill_expired_borrows(&last_use_map, bb, stmt_idx - 1)` | `kill_expired_borrows_dataflow(mir, &live_out, bb, stmt_idx - 1)` |
| Terminator kill | `kill_expired_borrows(&last_use_map, bb, term_idx)` | `kill_expired_borrows_dataflow(mir, &live_out, bb, term_idx)` |
| Region inference | `run_region_inference(mir)` | `run_region_inference(mir)` |

This deliberate symmetry lets us validate the dataflow path against the
legacy path on the same MIR shape (parity tests, see §6.2).

### 3.5 Why "live after point" instead of "last use at point"

The legacy analysis asks "is this the last use of `ref_local`?" — a
single-point check that misses loop back-edges (a local's "last use"
inside a loop body is not its true last use; the next iteration will
read it again).

The dataflow analysis asks "is `ref_local` live *after* this point?" —
a set-membership check that correctly handles loops (if `ref_local` is
live at the loop header, it's live throughout the loop body) and
conditionals (if `ref_local` is live in any successor branch, it's live
at the branch point).

### 3.6 Complexity

- `compute_live_after_point`: O((S - stmt_idx) × L) per call, where S =
  statements in `bb` and L = locals in the live set. For typical blocks
  (S<20, L<30) this is well under 100µs.
- `kill_expired_borrows_dataflow`: O(B_active × L) per call, where
  B_active = active borrows (typically <10).
- `check_mir_body_with_dataflow` total: O(B × (S × L + B_active × L)) =
  O(B × S × L) for typical functions (<50 blocks, <30 statements/block,
  <30 locals) — well under 1ms.

## 4. Implementation Notes

### 4.1 `active_ref_locals` deduplication

`BorrowSet::active_ref_locals` returns a deduplicated iterator over the
`ref_local`s of all active borrows. Two borrows can share the same
`ref_local` (e.g., `r = &x; r = &y;` rebinds `r`), so deduplication is
needed to avoid calling `kill_borrows_of_local` twice on the same local.
The implementation uses a `HashSet` for dedup, which is O(1) per insert.

### 4.2 `fold_use_def` ordering

The liveness transfer function `live = Use ∪ (live - Def)` must be
applied in the correct order: remove `defs` FIRST, then add `uses`. If
we added `uses` first, a local that's both read and written by the same
statement (e.g., `x = x + 1`) would incorrectly remain in the live set
even though the write makes the prior value dead.

### 4.3 No speculative API additions

Per §15 "最优 > 最小": we add only the minimum API the dataflow kill
path needs. No `kill_borrow_by_index`, no `live_at_point` (we have
`live_after_point` which is what the kill path needs), no separate
`compute_live_before_point` (callers can derive it from
`compute_live_after_point` + the statement's Use/Def). Each addition is
justified by a concrete call site.

### 4.4 Backward-compatibility with `check_mir_body`

The legacy `check_mir_body` is **retained unchanged**. The driver still
calls `check_mir_body`. The dataflow path is only invoked by the new
tests. This allows us to validate the dataflow path on real MIR for one
full stage before flipping the switch in Stage 15.37.

## 5. API Naming Compliance (§23)

| Symbol | Pattern | Status |
|--------|---------|--------|
| `check_mir_body_with_dataflow` | `<verb>_<noun>_<noun>` with `_with_dataflow` suffix (§23.1 rule 7 `check_` prefix) | ✅ |
| `check_mir_body_with_dataflow` (free fn) | `<verb>_<noun>_<noun>` (§23.1 rule 1 free-function pattern) | ✅ |
| `kill_expired_borrows_dataflow` | `<verb>_<noun>_<noun>` with `_dataflow` suffix (private method, but follows convention) | ✅ |
| `compute_live_after_point` | `<verb>_<noun>_<noun>_<noun>` (§23.1 rule 1 free-function pattern) | ✅ |
| `active_ref_locals` | `<verb>_<noun>_<noun>` (returns iterator over active ref_locals) | ✅ |
| `fold_use_def` | `<verb>_<noun>_<noun>` (private helper) | ✅ |

Per §23.1 rule 4: `borrowck::mod` uses explicit re-export list (no glob):

```rust
pub use liveness::{
    compute_last_use_map, compute_live_after_point, compute_liveness, successors, LastUseMap,
    LiveInMap, LiveOutMap,
};
```

Per §23.1 rule 5 (DRY): `compute_live_after_point` is defined in
`liveness.rs` (the liveness module); no duplicate definition elsewhere.

Per §23.1 rule 6 (deprecation): not applicable — no deprecated items in
this stage. The legacy `check_mir_body` is still the default; we'll
deprecate it in Stage 15.37 after the driver switches.

## 6. Testing

### 6.1 Unit tests (9 tests)

**`compute_live_after_point` tests (5, in `src/borrowck/liveness.rs::tests`):**

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_36_compute_live_after_point_at_terminator` | At stmt_idx == statements.len(), result equals LiveOut[bb_id] |
| 2 | `stage15_36_compute_live_after_point_folds_terminator` | At stmt_idx < term_idx, terminator's Use/Def is folded in |
| 3 | `stage15_36_compute_live_after_point_back_propagates` | Multiple statements are correctly back-propagated |
| 4 | `stage15_36_compute_live_after_point_out_of_range_bb` | Out-of-range bb_id returns empty set (no panic) |
| 5 | `stage15_36_compute_live_after_point_terminator_equals_live_out` | At term_idx, result equals LiveOut[bb_id] exactly |

**`active_ref_locals` tests (4, in `src/borrowck/borrow_set.rs::tests`):**

| # | Test name | Verifies |
|---|-----------|----------|
| 6 | `stage15_36_active_ref_locals_returns_distinct_set` | Returns distinct set of ref_locals (dedup) |
| 7 | `stage15_36_active_ref_locals_skips_none` | Borrows with `ref_local = None` are skipped |
| 8 | `stage15_36_active_ref_locals_empty_when_no_borrows` | Empty when no borrows are active |
| 9 | `stage15_36_active_ref_locals_after_kill` | After `kill_borrows_of_local`, killed ref_local no longer appears |

### 6.2 Integration tests (13 tests, `tests/v0/stage15/plan/kill_borrows_dataflow_tests.rs`)

**Part A — Smoke tests (4):**
1. `stage15_36_smoke_test_straight_line` — simple straight-line code
2. `stage15_36_smoke_test_control_flow` — if/else if/else
3. `stage15_36_smoke_test_loop` — while loop with mutation
4. `stage15_36_smoke_test_borrows` — `let r = &x`

**Part B — Parity tests (4, dataflow vs legacy):**
5. `stage15_36_parity_simple_program` — straight-line parity
6. `stage15_36_parity_control_flow_program` — control flow parity
7. `stage15_36_parity_loop_program` — loop parity (no borrows)
8. `stage15_36_parity_borrows_straight_line` — borrow parity

**Part C — Loop-borrow soundness tests (2, where legacy may fail):**
9. `stage15_36_loop_borrow_survives_across_iterations` — borrow created
   before loop, used inside loop — must survive across iterations
10. `stage15_36_branch_borrow_survives_both_arms` — borrow used in both
    arms of a conditional — must survive the branch

**Part D — `compute_live_after_point` integration tests (3):**
11. `stage15_36_compute_live_after_point_smoke_test` — callable on real
    MIR, no panic for any program point
12. `stage15_36_compute_live_after_point_terminator_eq_live_out` — at
    terminator index, result equals LiveOut[bb]
13. `stage15_36_smoke_test_complex_program` — mixed borrows + loops +
    conditionals

### 6.3 Regression strategy

- All 173 lib tests pass (zero regression) + 9 new unit tests = 182 lib
  tests.
- All 2026 integration tests pass (zero regression) + 13 new integration
  tests = 2039 integration tests.
- All 5216 conformance tests pass (zero regression) — the driver still
  uses the legacy `check_mir_body`, so conformance behavior is identical
  to v0.161.0.
- 0 clippy warnings, fmt clean.

## 7. Migration Plan (Stages 15.34-15.37)

| Stage | Status | Description |
|-------|--------|-------------|
| 15.34 | ✅ DONE (v0.160.0) | NLL fixpoint design doc |
| 15.35 | ✅ DONE (v0.161.0) | `compute_liveness` fixpoint function |
| **15.36** | **✅ DONE (v0.162.0)** | **`kill_expired_borrows_dataflow` + `check_mir_body_with_dataflow` (this stage)** |
| 15.37 | ⏳ NEXT | Switch driver to use `check_mir_body_with_dataflow` + remove `compute_last_use_map` |

## 8. Verification

- `cargo build --features llvm-backend` — ✅ clean build, 0 warnings
- `cargo test --features llvm-backend --lib borrowck::liveness::tests::stage15_36` — ✅ 5/5 unit tests pass
- `cargo test --features llvm-backend --lib borrowck::borrow_set::tests::stage15_36` — ✅ 4/4 unit tests pass
- `cargo test --features llvm-backend --test all_tests stage15_kill_borrows_dataflow_tests` — ✅ 13/13 integration tests pass
- All existing tests pass (zero regression — verified by full `cargo test` run)

## 9. Stage Gate Review (self-review per §9.3)

| Check | Status |
|-------|--------|
| Design doc exists (`docs/lang-design/23-nll-fixpoint.md`) | ✅ |
| Implementation matches design | ✅ |
| Unit tests cover algorithm internals | ✅ 9 tests |
| Integration tests cover real pipeline + soundness patterns | ✅ 13 tests |
| API naming compliance (§23) | ✅ |
| §16 interface isolation | ✅ — `kill_expired_borrows_dataflow` reads only `mir` and `live_out`, no writes/HIR lookup |
| §15 最优 > 最小 — no speculative API additions | ✅ |
| 0 clippy warnings | ✅ |
| fmt clean | ✅ |
| Zero regression on existing tests | ✅ |

## 10. Conclusion

Stage 15.36 lands the dataflow-driven borrow-checker entry point. The
new `check_mir_body_with_dataflow` method uses the fixpoint liveness
analysis from Stage 15.35 to expire borrows, correctly handling loops
and conditionals. The legacy `check_mir_body` is retained for one stage
so we can A/B-test the dataflow path on real MIR before flipping the
switch.

The next stage (15.37) will:
1. Switch the driver to call `check_mir_body_with_dataflow`.
2. Remove `compute_last_use_map` and `kill_expired_borrows` (dead code).
3. Deprecate `check_mir_body` (the legacy entry point) — callers should
   use `check_mir_body_with_dataflow` instead.
