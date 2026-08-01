# v0.2 Phase 2: Fixpoint Dataflow NLL Design

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.159.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 2 Task 7**: Activate fixpoint dataflow NLL (HP-10)

## 1. Problem Statement

The current borrow checker uses a **single-pass last-use map** (Stage 6.14):
- Forward scan records the last read point for each local
- Borrows are killed when the borrowed local reaches its last use
- This is **not sound** for complex control flow (loops, conditionals)

### Issues with the current approach:
1. **Loops**: A local's "last use" in a loop body is not its true last use —
   it may be used again in the next iteration. The current approach kills
   borrows too early.
2. **Conditionals**: A borrow alive in one branch may still be live in the
   other branch. The current approach doesn't track branch liveness.
3. **No fixpoint**: The analysis is a single forward pass — it doesn't
   iterate to convergence.

## 2. Design: Fixpoint Dataflow NLL

### 2.1 Overview

Replace the single-pass `compute_last_use_map` with a **fixpoint liveness
analysis** that iterates until convergence:

```
LiveIn[bb] = Use[bb] ∪ (LiveOut[bb] - Def[bb])
LiveOut[bb] = ∪ LiveIn[succ] for succ in successors(bb)
```

This is the standard backwards dataflow analysis for liveness. It correctly
handles loops (a variable is live if it's used in any successor, including
the loop header) and conditionals (a variable is live if it's used in any
branch).

### 2.2 Data Structures

```rust
/// Liveness map: BasicBlockId → set of live locals at block entry.
pub type LiveInMap = HashMap<BasicBlockId, HashSet<LocalId>>;

/// Liveness map: BasicBlockId → set of live locals at block exit.
pub type LiveOutMap = HashMap<BasicBlockId, HashSet<LocalId>>;

/// Compute liveness via fixpoint iteration.
pub fn compute_liveness(mir: &MirBody) -> (LiveInMap, LiveOutMap) {
    // Initialize all blocks to empty sets
    // Iterate backwards until no changes (fixpoint)
    // ...
}
```

### 2.3 Borrow Expiry

Instead of killing borrows at "last use" (single point), kill borrows when
the borrowed local is no longer live at the current program point:

```rust
fn kill_expired_borrows_dataflow(
    &mut self,
    live_in: &LiveInMap,
    bb_id: BasicBlockId,
    stmt_idx: usize,
) {
    // A borrow of local L is expired at point P if L is not live after P.
    // ...
}
```

### 2.4 Migration Strategy

1. **Stage 15.34**: Add `compute_liveness` fixpoint function (keep `compute_last_use_map` for backward compat)
2. **Stage 15.35**: Add `kill_expired_borrows_dataflow` using liveness maps
3. **Stage 15.36**: Switch borrow checker to use fixpoint liveness (replace last-use map)
4. **Stage 15.37**: Remove `compute_last_use_map` (dead code)

### 2.5 Testing

- All existing borrowck tests must pass (no regression)
- New tests for loop + borrow patterns
- New tests for conditional + borrow patterns

## 3. Dependencies

- None — fixpoint NLL is independent of other Phase 2 tasks
- Unblocks: Drop elaboration (Task 8), Region allocation (Task 9)

## 4. Effort

- 1-2 weeks (per v0.2-preparation.md)
- Stage 15.34-15.37: ~4 stages, each independently testable
