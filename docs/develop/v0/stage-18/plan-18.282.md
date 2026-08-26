# Stage 18.282 — TD-DROP-MOVED-LOCALS Full: Flow-Sensitive Move Tracking Design

> **Author**: Super Z (main) — PM-A + ARCH-A
> **Date**: 2026-08-25
> **Version**: v0.492.0 (no bump — design)
> **Process**: stage-committee-process.md v7.3 §13.4 (重构即架构设计) + §13.5 (设计-审查循环) + §18 (依赖审查)
> **Status**: Design — awaiting REV-A review

---

## 1. Problem Statement

Current `collect_moved_locals` (drop_elaboration.rs:74) is **flow-insensitive**: it scans ALL basic blocks and collects ANY local that is moved via `Operand::Move`. This means:
- A local moved in BB1 but NOT moved in BB2 (conditional path) is still marked as "moved" globally
- Drop elaboration skips drop for this local in ALL paths, including BB2 where it was never moved
- This can cause **missing drops** (memory leak / resource leak) on paths where the local was NOT moved

### Example (unsound with flow-insensitive):
```rust
fn f(cond: bool) {
    let s = String::from_str("hello");  // s needs drop
    if cond {
        consume(s);  // s is moved — no drop needed on this path
    } else {
        // s is NOT moved here — drop IS needed on this path
        // But flow-insensitive marking says "s is moved" → drop is SKIPPED → LEAK!
    }
}
```

---

## 2. §18 Dependency Audit Results

### Infrastructure capability: ✅ COMPLETE

| Capability | Location | Reusable? |
|-----------|----------|-----------|
| Backwards dataflow fixpoint | `borrowck/liveness.rs:101` `compute_liveness()` | ✅ Pattern reusable |
| Per-BB use/def sets | `liveness.rs:117-140` `statement_reads/writes` | ✅ Can extend for move detection |
| Fixpoint iteration loop | `liveness.rs:145-170` `while changed` | ✅ Same pattern |
| Move operand detection | `drop_elaboration.rs:141` `collect_moved_locals_from_operand` | ✅ Reuse as transfer function |
| Drop elaboration integration | `drop_elaboration.rs:480` `elaborate_drops()` | ✅ Replace `collect_moved_locals` call |

---

## 3. Design: Flow-Sensitive Move State Analysis

### 3.1 Data Structure

```rust
/// Per-block moved-state maps (forwards dataflow).
/// `moved_in[B]` = set of locals moved on ALL paths reaching B (intersection of preds' moved_out)
/// `moved_out[B]` = moved_in[B] ∪ {locals moved in B's statements/terminator}
pub type MovedInMap = HashMap<BasicBlockId, HashSet<LocalId>>;
pub type MovedOutMap = HashMap<BasicBlockId, HashSet<LocalId>>;
```

### 3.2 Algorithm (Forwards Dataflow Fixpoint)

```rust
pub fn compute_moved_state(mir: &MirBody) -> (MovedInMap, MovedOutMap) {
    // 1. Initialize: all blocks → empty set
    // 2. Pre-compute per-block move sets (locals moved via Operand::Move)
    // 3. Fixpoint: forwards iteration
    //    moved_out[B] = moved_in[B] ∪ block_moves[B]
    //    moved_in[B] = ∩ moved_out[P] for P in preds(B)
    //    (entry block: moved_in = ∅)
    // 4. Converge when no moved_in changes
}
```

### 3.3 Integration with elaborate_drops

Replace:
```rust
let moved_locals = collect_moved_locals(mir);  // flow-insensitive
```
With:
```rust
let (moved_in, moved_out) = compute_moved_state(mir);  // flow-sensitive
// In the drop insertion loop, check moved_out[current_bb] instead of global set
```

### 3.4 Fallback

Keep `collect_moved_locals` as fallback for edge cases (e.g., when `compute_moved_state` returns empty due to no moves). Per §2.2 原则 4 (报错 > 静默): if moved_state computation fails, fall back to conservative (flow-insensitive) rather than silently dropping nothing.

---

## 4. §13.4 J1-J6 Audit

| # | Judgment | Verification |
|---|----------|-------------|
| J1 | Architecture alignment | ✅ Mirrors `compute_liveness` pattern (§25-drop-elaboration.md design) |
| J2 | Single responsibility | ✅ Move tracking is independent analysis |
| J3 | One-way flow | ✅ MIR → moved_state → elaborate_drops (one direction) |
| J4 | Compile-concept completeness | ✅ Self-contained dataflow analysis |
| J5 | Stage division | ✅ MIR layer, no cross-stage calls |
| J6 | Reasonable size | ✅ ~200 LOC new + ~50 LOC modified |

---

## 5. Test Plan (§9.4.3 — 1:3+ ratio)

| # | Type | Test |
|---|------|------|
| 1 | Positive | `let s = String::from_str("x"); consume(s);` — s is moved, no drop leak |
| 2 | Positive | Conditional move: `if cond { consume(s); }` — s dropped on else path |
| 3 | Negative | Conditional move + no drop on else → verify drop IS inserted |
| 4 | Negative | Loop with move → verify drop NOT inserted on re-entry path |
| 5 | Negative | Nested conditional → verify correct intersection semantics |
| 6 | Negative | Multiple moves of same local → verify only first path marks as moved |

Ratio: 2 positive + 4 negative = 1:3 ✅
