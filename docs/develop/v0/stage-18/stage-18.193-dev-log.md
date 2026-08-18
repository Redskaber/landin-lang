# Stage 18.193 — Task Review: Box Auto-Drop (DEFERRED)

> **Date**: 2026-08-17
> **Version**: v0.460.0 → v0.460.0 (no code change)
> **Task ID**: stage18.193

## 1. Task: Box Auto-Drop

Attempted to implement auto-drop for Box<T> — when a Box goes out of scope,
automatically call `__landin_dealloc` on the inner pointer.

## 2. Blocker: TD-DROP-MOVED-LOCALS

**Root cause**: Box::new intrinsic creates a `FnDef`-typed local to hold the
function reference to `__landin_alloc`. This local has the same LLVM layout
as Box (`{ ptr }`), because both FnDef and Box map to `OpaquePtr`/`{ ptr }`
in the LLVM type system.

When `ty_needs_drop` marks Box as needing drop, the FnDef local is also
marked (incorrectly), and the drop elaboration inserts a `Drop` terminator
on it. The drop glue then tries to `__landin_dealloc` the FnDef constant
value (integer 16), causing a segfault.

**Why this is hard to fix**: The drop elaboration doesn't track which locals
have been moved from. After `let b = Box::new(x)`, the FnDef local that
held the function reference should be considered "moved" and not dropped.
But the current drop elaboration doesn't have this capability.

**Proper fix** (TD-DROP-MOVED-LOCALS): Drop elaboration needs to track
move state per local — only insert Drop terminators for locals that haven't
been moved from. This is a significant refactor of the drop elaboration pass.

## 3. Decision: DEFER

Per §17 task review: the Box auto-drop task is blocked by TD-DROP-MOVED-LOCALS.
The proper fix (move tracking in drop elaboration) is a v0.3+ feature.
For now, Box users must manually call `__landin_dealloc(b.0 as *mut u8)`.

## 4. TD Record

| ID | Description | Priority | Fix Plan |
|----|-------------|----------|----------|
| TD-BOX-AUTO-DROP | Box no auto-drop — users must manually dealloc | P2 | v0.3+: after TD-DROP-MOVED-LOCALS |
| TD-DROP-MOVED-LOCALS | Drop elaboration doesn't track moved-from locals | P2 | v0.3+: add move state tracking to drop elaboration |

## 5. §3.2 Acceptance (no code change)

- ✅ cargo test: 658 lib + 3049 integration = 3707 total, 0 failures
