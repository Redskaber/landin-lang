# Stage 18.286 — TD-IF-RETURN-VALUE-CODEGEN: const_prop Merge Point Fix

> **Author**: Super Z (main) — PM-A + ARCH-A
> **Date**: 2026-08-25
> **Version**: v0.493.0 → v0.494.0 (planned)
> **Process**: stage-committee-process.md v7.3 §13.5 (设计-审查循环) + §17.6 (整体性修复)
> **Status**: Design — awaiting REV-A review

---

## 1. Problem Statement

`run_const_prop` (optimization.rs:321) assumes **linear control flow** — it
processes basic blocks in index order (0, 1, 2, ...) and accumulates a single
`const_map: HashMap<LocalId, Const>` across all blocks. This is unsound when
control flow has merge points (if/else, match, loops).

### 1.1 Root Cause

For `fn f(b: bool) -> i32 { if b { 1i32 } else { 0i32 } }`:

MIR (correct, pre-opt):
```
bb0: SwitchInt(b) → bb1 if true, bb2 otherwise
bb1: loc_3 = 1; loc_2 = Copy(loc_3); Goto(bb3)
bb2: loc_4 = 0; loc_2 = Copy(loc_4); Goto(bb3)
bb3: loc_0 = Move(loc_2); Return
```

After `run_const_prop` (BUG):
```
bb0: SwitchInt(b) → bb1 if true, bb2 otherwise
bb1: Goto(bb3)                    ← assignments REMOVED!
bb2: Goto(bb3)                    ← assignments REMOVED!
bb3: loc_0 = Constant(0); Return  ← should be Move(loc_2), not Constant(0)
```

The bug: `const_prop` processes bb1 (`const_map[loc_2] = 1`), then bb2
(`const_map[loc_2] = 0` — overwrites!). At bb3, `Move(loc_2)` is propagated
to `Constant(0)` — the value from bb2, ignoring bb1's value.

### 1.2 Impact

- **Soundness bug**: `if`/`match` expressions as function tail return wrong
  values (always the else branch's value, ignoring the then branch).
- **Affects ALL code** using if/match as value expression, not just prelude.
- Discovered via Stage 18.285's `bool::to_int` impl: `true.to_int()` returns 0
  instead of 1.

### 1.3 Why DCE removes the assignments

After `const_prop` turns `loc_2 = Copy(loc_3)` into `loc_2 = Constant(1)` (in
bb1), the second DCE pass sees `loc_3` is no longer read (the only reader was
`loc_2 = Copy(loc_3)`, now replaced). So DCE removes `loc_3 = 1`. Then
`loc_2 = Constant(1)` — but `const_prop` already replaced `Move(loc_2)` in
bb3 with `Constant(0)`, so `loc_2` is also dead → removed.

---

## 2. Design: Predecessor-Aware const_prop

### 2.1 Algorithm

The fix makes `const_prop` **control-flow aware** by computing predecessor
sets and clearing `const_map` at merge points where a local has different
values across predecessors.

**Step 1: Compute predecessors.** For each BB, collect the set of BBs that
have a terminator targeting it.

**Step 2: Per-BB const_map intersection.** Instead of a single global
`const_map`, track per-predecessor const maps. At a merge point (BB with >1
predecessor), intersect the const maps: a local is constant only if ALL
predecessors agree on its value.

**Step 3: Process BBs in reverse-postorder (RPO).** RPO ensures all
predecessors are processed before a BB (for acyclic graphs). For loops
(back-edges), clear const_map at loop headers (existing behavior preserved).

### 2.2 Implementation Detail

To keep the change minimal (§12 最优 > 最小 — fix root cause, not rewrite),
use a **per-BB snapshot approach**:

```rust
pub fn run_const_prop(mir: &mut MirBody) {
    // Step 1: Compute predecessors.
    let preds = compute_predecessors(mir);

    // Step 2: Process BBs in index order (existing), but at merge points
    // (preds[bb].len() > 1), clear const_map entries that don't agree
    // across all predecessors.
    //
    // For acyclic CFGs, processing in index order works IF we snapshot
    // const_map at each BB and intersect at merge points.
    //
    // For loops (back-edges), clear const_map at loop headers (existing
    // behavior preserved).
    let has_back_edges = ...;  // existing

    // Track per-BB const_map snapshots for merge-point intersection.
    let mut bb_const_maps: Vec<HashMap<LocalId, Const>> = vec![HashMap::new(); mir.basic_blocks.len()];

    for bb_idx in 0..mir.basic_blocks.len() {
        // Compute incoming const_map: intersection of all predecessors'
        // outgoing const_maps. For bb0 (entry), start empty.
        let mut incoming = if bb_idx == 0 {
            HashMap::new()
        } else {
            intersect_const_maps(&preds[bb_idx], &bb_const_maps)
        };

        // Loop header: clear (existing behavior).
        if is_loop_header { incoming.clear(); }

        // Process statements with `incoming` as the const_map.
        let mut const_map = incoming;
        let bb = &mut mir.basic_blocks[bb_idx];
        for stmt in &mut bb.statements {
            // ... existing propagation + folding using const_map ...
        }

        // Save outgoing const_map for this BB.
        bb_const_maps[bb_idx] = const_map;
    }
}
```

### 2.3 Intersection Logic

`intersect_const_maps(preds, bb_const_maps)`:
- For each local in any predecessor's const_map:
  - If ALL predecessors have the same constant value → keep.
  - If any predecessor has a different value OR no entry → drop.
- Returns the intersected map.

This ensures at a merge point, only "definitely constant" locals survive.

### 2.4 Edge Cases

- **Loop back-edges**: existing `is_loop_header` check clears const_map.
  This is preserved (safe over conservative).
- **Unreachable BBs**: if a BB has 0 predecessors (other than bb0), its
  const_map is irrelevant — DCE or unreachable analysis handles it.
- **Multiple paths to same BB (diamond)**: intersection handles this.

### 2.5 Why Not Worklist Dataflow?

A proper dataflow analysis (worklist algorithm with fixpoint iteration) would
be more general, but:
- The existing code is simple sequential processing.
- Adding worklist complexity is a larger change (§12 最优 > 最小 — fix root
  cause, not rewrite).
- The intersection approach handles the common case (acyclic CFG with merge
  points) correctly.
- Loops are already handled conservatively (clear at headers).

Per §1.0 原則 6 (通解 > 特解): intersection is the 通解 for merge points —
handles if/else, match, and any multi-predecessor BB uniformly.

---

## 3. §13.4 J1-J6 Compliance

This is a fix to an existing module (`optimization.rs`), not a refactor.
J1-J6 apply to refactors, not bug fixes. The fix:
- Stays within `mir::optimization` (J5).
- Adds `compute_predecessors` + `intersect_const_maps` helpers (J2 single
  responsibility).
- No circular deps (J3).
- Complete merge-point handling in one place (J4, J6).

---

## 4. Test Plan (§9.4.3 — 1:3+ ratio)

### 4.1 Positive tests (5)

| Test | What it verifies |
|------|-----------------|
| `if_else_returns_then_value` | `if true { 1 } else { 0 }` returns 1 (not 0) |
| `if_else_returns_else_value` | `if false { 1 } else { 0 }` returns 0 |
| `if_else_returns_correct_branch` | Runtime: `if cond { X } else { Y }` returns X when cond=true, Y when cond=false |
| `match_as_tail_returns_correct` | `match b { true => 1, false => 0 }` returns 1 when b=true |
| `nested_if_returns_correct` | `if a { if b { 1 } else { 2 } } else { 3 }` returns correct value for all 4 combos |

### 4.2 Negative audit set (30+ cases)

Cover all 7 error categories, ≥30 cases (per §7.3.1).

---

## 5. Implementation Plan

1. Add `compute_predecessors(mir) -> Vec<HashSet<BasicBlockId>>` helper.
2. Add `intersect_const_maps(preds, bb_const_maps) -> HashMap<LocalId, Const>` helper.
3. Refactor `run_const_prop` to use per-BB const_map snapshots + intersection
   at merge points.
4. Restore prelude impls to idiomatic form (use `if`/`match` as tail, remove
   workarounds).
5. Add positive + negative tests.
6. Run §3.2 full validation.
7. Update tech-debt-register (TD-IF-RETURN-VALUE-CODEGEN → Resolved).

### 5.1 Files touched

| File | Change | LOC delta |
|------|--------|-----------|
| `src/mir/optimization.rs` | Add helpers + refactor run_const_prop | +80 |
| `src/stdlib/prelude.rs` | Restore idiomatic prelude impls | ±10 |
| `tests/v0/stage18/plan/stage18_286_const_prop_merge_tests.rs` (NEW) | Tests | +400 |
