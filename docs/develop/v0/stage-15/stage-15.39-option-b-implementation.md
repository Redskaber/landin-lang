# Stage 15.39 — Option B Implementation (`compute_ever_read` + GAP-1 preservation)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.164.0 → v0.165.0
> **Process**: stage-committee-process.md v3.23 §13.4 + §29.5
> **v0.2 Phase 2 Task 7 (step 4)**: Implement Option B from reconciliation design doc
> **Design doc**: `docs/lang-design/24-gap1-reconciliation.md`
> **Prior stage**: `docs/develop/v0/stage-15/stage-15.38-borrowck-comparison-tool.md`

## 1. Executive Summary

Stage 15.39 implements **Option B** from the GAP-1 reconciliation design
doc: a `compute_ever_read` pre-pass that computes the set of locals read
anywhere in the MIR body, plus a modified `kill_expired_borrows_dataflow`
that uses this set to skip killing borrows whose `ref_local` was never
read. This preserves the legacy path's "stray borrow" behavior, making
the dataflow path reject the same GAP-1 patterns the legacy path rejects.

**Key result**: The diagnostic tool (Stage 15.38) confirms the
LEGACY-STRICTER count dropped from **112 → 0** — the GAP-1 conflict is
resolved. The dataflow path now agrees with the legacy path on all 112
GAP-1 cases.

**Known limitation**: 1 DATAFLOW-STRICTER case remains (a false positive
on `&mut self` method calls in loops). This is a separate bug from the
GAP-1 conflict and is deferred to a future stage.

Per §1.0 原則 1 "长期 > 短期": Option B is the right long-term design
for now — it fixes the real NLL soundness bugs (loops, conditionals)
without changing the project's soundness posture. Per §1.0 原則 5
"报错 > 静默": preserving the stray borrow is the safer choice.

## 2. What Was Done

### 2.1 Added `compute_ever_read` to `src/borrowck/liveness.rs`

```rust
/// Compute the set of locals that are read anywhere in the MIR body.
pub fn compute_ever_read(mir: &MirBody) -> HashSet<LocalId> {
    let mut ever = HashSet::new();
    for bb in &mir.basic_blocks {
        for stmt in &bb.statements {
            for local in statement_reads(stmt) {
                ever.insert(local);
            }
        }
        for local in terminator_reads(&bb.terminator) {
            ever.insert(local);
        }
    }
    ever
}
```

The function is a simple forward scan that collects every local
appearing in any `statement_reads` or `terminator_reads` result. The
resulting set is used by `kill_expired_borrows_dataflow` to preserve
GAP-1 semantics.

### 2.2 Modified `kill_expired_borrows_dataflow` in `src/borrowck/mod.rs`

Added an `ever_read: &HashSet<LocalId>` parameter. The kill logic now
checks:

1. **GAP-1 preservation**: If the `ref_local` was never read anywhere in
   the body (`!ever_read.contains(local)`), do NOT kill the borrow. The
   borrow stays as a "stray" until scope end, matching the legacy path's
   behavior.
2. **NLL kill**: If the `ref_local` was read AND is not live after the
   current point, kill the borrow (standard NLL).

```rust
let locals_to_kill: Vec<LocalId> = self
    .borrows
    .active_ref_locals()
    .filter(|local| {
        // GAP-1 preservation: never kill a borrow whose ref_local
        // was never read.
        if !ever_read.contains(local) {
            return false; // do NOT kill
        }
        // NLL: kill if the ref_local is not live after this point.
        !live_after.contains(local)
    })
    .collect();
```

### 2.3 Updated `check_mir_body_with_dataflow` to compute and pass `ever_read`

```rust
pub fn check_mir_body_with_dataflow(&mut self, mir: &MirBody) {
    let (_live_in, live_out) = compute_liveness(mir);
    let ever_read = compute_ever_read(mir);  // NEW

    for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
        // ... pass &ever_read to kill_expired_borrows_dataflow ...
    }
}
```

### 2.4 Re-exported `compute_ever_read` from `borrowck/mod.rs`

```rust
pub use liveness::{
    compute_ever_read, compute_last_use_map, compute_live_after_point, compute_liveness,
    successors, LastUseMap, LiveInMap, LiveOutMap,
};
```

## 3. Why Option B Works

### 3.1 The GAP-1 conflict root cause

The Stage 14.81 GAP-1 fix decided that `let r1 = &mut x; let r2 = &mut x;`
should be a `compile_error` even when `r1` is never read after `r2` is
created. The legacy path achieves this "accidentally":

- `compute_last_use_map` only records locals that are **read**.
- A never-read local (like `r1`) never appears in the map.
- `kill_expired_borrows` only kills borrows whose `ref_local` has a
  recorded "last use" — so a never-read local's borrow is NEVER killed.
- The borrow stays alive as a "stray" and conflicts with the new borrow.

The dataflow path (before Option B) correctly identifies `r1` as dead
(never read after assignment) and kills its borrow — allowing `r2 = &mut x`
to succeed. This is correct NLL but violates GAP-1.

### 3.2 How Option B fixes it

Option B adds the "was ever read" check: if a `ref_local` was never read
ANYWHERE in the body, its borrow is NOT killed (matching the legacy
path's "stray borrow" behavior). This preserves GAP-1 while still using
the dataflow infrastructure for loop/conditional correctness.

For `let r1 = &mut x; let r2 = &mut x;`:
- `r1` is never read → `r1` is NOT in `ever_read`.
- The "was ever read" check skips killing `r1`'s borrow.
- `r1`'s borrow stays alive as a "stray".
- `r2 = &mut x` conflicts with the stray → `compile_error` (GAP-1 preserved).

### 3.3 Why loop-borrow soundness is preserved

For `let r = &x; while i < 3 { s += *r; i += 1; }`:
- `r` IS read (inside the loop body, via `*r`).
- `r` is in `ever_read`.
- The "was ever read" check does NOT skip killing `r`'s borrow.
- The normal NLL liveness check applies: `r` is live across the loop
  (used in the loop body), so its borrow is NOT killed.
- The loop-carried borrow works correctly (soundness preserved).

## 4. Diagnostic Verification

Re-ran the Stage 15.38 diagnostic tool after implementing Option B:

```
Files scanned: 5216 (skipped: 188)
Files compared: 5028
  AGREE-OK:           4829
  AGREE-ERROR:        198   (was 86 before — the 112 GAP-1 cases moved here)
  LEGACY-STRICTER:    0     (was 112 — GAP-1 conflict RESOLVED)
  DATAFLOW-STRICTER:  1     (unchanged — known limitation)
  DIFFERENT-ERRORS:   0
```

**The 112 LEGACY-STRICTER cases moved to AGREE-ERROR** — both paths now
reject them with the same error count. The GAP-1 conflict is resolved.

## 5. Known Limitation: `&mut self` Method-Call False Positive

The 1 remaining DATAFLOW-STRICTER case is
`e2e-runok-132-state-machine.lin` — a valid `run_ok` program with
`&mut self` method call chains in a loop. The dataflow path rejects it
with a false positive.

### 5.1 Root cause

In a loop like:
```rust
while i < 5 {
    c.increment();  // &mut c borrow, ref_local = tmp
    i = i + 1;
}
```

The MIR lowering creates a fresh `tmp = &mut c` each iteration. The
dataflow liveness analysis correctly identifies `tmp` as live across the
loop back-edge (it's used in the next iteration's call). This means
`tmp`'s borrow is never killed by the dataflow path — it stays alive
and conflicts with the next iteration's `&mut c` borrow.

The legacy path doesn't have this problem because `compute_last_use_map`
records `tmp`'s last use at the call point (within the same iteration),
so `kill_expired_borrows` kills `tmp`'s borrow at the call's terminator.

### 5.2 Why Option B doesn't fix it

Option B's "was ever read" check only applies to never-read locals.
`tmp` IS read (by the call), so it's in `ever_read`, and the normal NLL
liveness check applies. The NLL liveness correctly says `tmp` is live
across the back-edge, so the borrow is not killed.

### 5.3 The correct fix (deferred)

The correct fix is to kill a borrow when its `ref_local` is **redefined**
(re-assigned), not just when it becomes dead. In the loop, `tmp` is
re-assigned at the start of each iteration (`tmp = &mut c`). The
borrow-check walk should kill `tmp`'s old borrow before processing the
re-assignment.

This requires modifying the borrow-check walk to kill borrows at
re-definition points (in addition to the current "kill at last-use"
behavior). This is a deeper change to the walk structure and is
deferred to a future stage.

### 5.4 Impact on driver switch

The 1 false positive means the driver CANNOT switch to the dataflow path
yet — it would break 1 conformance test. The driver switch (Stage 15.40)
is blocked until this false positive is fixed.

However, the false positive is a single test case, and the fix is
well-understood (kill at re-definition points). The driver switch can
proceed once the fix lands.

## 6. API Naming Compliance (§23)

| Symbol | Pattern | Status |
|--------|---------|--------|
| `compute_ever_read` | `<verb>_<noun>_<noun>` (§23.1 rule 1 free-function pattern) | ✅ |
| `kill_expired_borrows_dataflow` (modified) | `<verb>_<noun>_<noun>` with `_dataflow` suffix (private method) | ✅ |
| `check_mir_body_with_dataflow` (modified) | `<verb>_<noun>_<noun>` with `_with_dataflow` suffix | ✅ |

Per §23.1 rule 4: `borrowck::mod` uses explicit re-export list (no glob).
Per §23.1 rule 5 (DRY): `compute_ever_read` is defined in `liveness.rs`
(the liveness module); no duplicate definition.

## 7. Testing

### 7.1 New unit tests (5, in `src/borrowck/liveness.rs::tests`)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_39_compute_ever_read_collects_all_reads` | Collects locals read in statements |
| 2 | `stage15_39_compute_ever_read_includes_terminator_reads` | Includes locals read by terminators (SwitchInt discr) |
| 3 | `stage15_39_compute_ever_read_empty_when_no_reads` | Empty set when only writes (no reads) |
| 4 | `stage15_39_compute_ever_read_empty_body` | Empty set for empty body (no panic) |
| 5 | `stage15_39_compute_ever_read_multiple_blocks` | Collects reads across multiple basic blocks |

### 7.2 New integration tests (9, in `tests/v0/stage15/plan/option_b_implementation_tests.rs`)

**Part A — GAP-1 preservation (3 tests, the main goal of Option B):**
1. `stage15_39_option_b_preserves_gap1_double_mut_borrow` — `let r1 = &mut x; let r2 = &mut x;` rejected by both paths
2. `stage15_39_option_b_preserves_gap1_shared_then_mut` — `let r = &x; let r2 = &mut x;` rejected by both paths
3. `stage15_39_option_b_preserves_gap1_borrow_then_mutate_after_scope` — `{ let r = &x; } x = 2;` rejected by both paths

**Part B — Loop-borrow soundness (1 test, preserved from Stage 15.36):**
4. `stage15_39_option_b_preserves_loop_borrow_soundness` — loop-carried borrow accepted by dataflow path

**Part C — Parity on valid programs (2 tests):**
5. `stage15_39_option_b_parity_valid_program` — both paths agree on valid program
6. `stage15_39_option_b_parity_single_borrow` — both paths agree on single borrow

**Part D — `compute_ever_read` public API (2 tests):**
7. `stage15_39_compute_ever_read_callable_on_real_mir` — callable on real MIR, no panic
8. `stage15_39_compute_ever_read_empty_for_no_reads` — empty set for no-read program

**Part E — Known limitation (1 test, documents the false positive):**
9. `stage15_39_known_limitation_mut_self_method_call_in_loop` — documents the `&mut self` false positive; will be updated when the fix lands

### 7.3 Regression strategy

- All 173 lib tests pass (zero regression) + 5 new = 178 lib tests.
- All 2052 integration tests pass (zero regression) + 9 new = 2061 integration tests.
- All 5216 conformance tests pass (zero regression — driver unchanged).
- 0 clippy warnings, fmt clean.

## 8. Verification

- `cargo build --features llvm-backend` — ✅ clean build, 0 warnings
- `cargo test --features llvm-backend --lib borrowck::liveness::tests::stage15_39` — ✅ 5/5 PASS
- `cargo test --features llvm-backend --test all_tests stage15_option_b_implementation_tests` — ✅ 9/9 PASS
- Diagnostic tool re-run: LEGACY-STRICTER dropped from 112 → 0 ✅
- All existing tests pass (zero regression)

## 9. Stage Gate Review (self-review per §9.3)

| Check | Status |
|-------|--------|
| Option B implemented per design doc | ✅ `compute_ever_read` + modified `kill_expired_borrows_dataflow` |
| GAP-1 conflict resolved (112 → 0) | ✅ diagnostic tool confirms |
| Loop-borrow soundness preserved | ✅ Stage 15.36 tests still pass |
| Known limitation documented | ✅ `&mut self` false positive, deferred to future stage |
| API naming compliance (§23) | ✅ |
| §16 interface isolation | ✅ — `compute_ever_read` reads only `&MirBody` |
| §15 最优 > 最小 — `compute_ever_read` is the minimum needed | ✅ |
| 0 clippy warnings | ✅ |
| fmt clean | ✅ |
| Zero regression on existing tests | ✅ |

## 10. Conclusion

Stage 15.39 successfully implements Option B from the GAP-1 reconciliation
design doc. The `compute_ever_read` pre-pass + modified
`kill_expired_borrows_dataflow` preserves GAP-1 semantics in the dataflow
path, resolving the 112-case conflict that blocked the driver switch in
Stage 15.37.

The diagnostic tool confirms LEGACY-STRICTER dropped from 112 → 0. The
dataflow path now agrees with the legacy path on all GAP-1 patterns.

One known limitation remains: a false positive on `&mut self` method
calls in loops (1 conformance case). This is a separate bug from the
GAP-1 conflict — the dataflow path's liveness analysis correctly
identifies the borrow temp as live across the loop back-edge, but the
borrow should be killed at the re-definition point. The fix is deferred
to a future stage.

**The driver switch (Stage 15.40) is still blocked** by the 1 false
positive. Once the false positive is fixed, the driver can switch to
`check_mir_body_with_dataflow` and the NLL migration will be complete.

## 11. Migration Plan (Stages 15.34-15.41) — Updated

| Stage | Status | Description |
|-------|--------|-------------|
| 15.34 | ✅ DONE (v0.160.0) | NLL fixpoint design doc |
| 15.35 | ✅ DONE (v0.161.0) | `compute_liveness` fixpoint function |
| 15.36 | ✅ DONE (v0.162.0) | `kill_expired_borrows_dataflow` + `check_mir_body_with_dataflow` |
| 15.37 | ⚠️ PARTIAL (v0.163.0) | Legacy `check_mir_body` deprecated; driver switch DEFERRED |
| 15.38 | ✅ DONE (v0.164.0) | Diagnostic tool + reconciliation design doc |
| **15.39** | **✅ DONE (v0.165.0)** | **Option B: `compute_ever_read` + GAP-1 preserved (112 → 0)** |
| 15.40 | ⏳ BLOCKED | Fix `&mut self` false positive, then switch driver |
| 15.41 | ⏳ PLANNED | Remove legacy `compute_last_use_map` + `kill_expired_borrows` + `check_mir_body` |
