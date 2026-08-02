# Stage 15.55 — Phase 3 Design Alignment (Task Selection + Planning)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.180.0 → v0.181.0
> **Process**: stage-committee-process.md v3.23 §13.4 (设计对齐) + §29
> **v0.2 Phase 3 (step 1)**: Feature Work — task selection + planning

## 1. Executive Summary

Stage 15.55 is a **design alignment** stage for v0.2 Phase 3 (Feature Work).
It assesses the readiness of each Phase 3 task, selects the first task to
implement, and creates a staged implementation plan.

**Decision**: Start Phase 3 with **Task 13 (`impl Drop` + RAII types)**.
This task has the lowest effort (1 week), the infrastructure is fully in
place (Drop elaboration from Stages 15.42-15.46), and it provides
immediate user-visible value (RAII types like `File`, `Box<T>`).

## 2. Phase 3 Task Readiness Assessment

| Task | Effort | Priority | Dependencies | Readiness |
|------|--------|----------|--------------|-----------|
| 11: Monomorphization | 2-3 weeks | P0 | Tasks 1-3 | ⚠️ Blocked (Task 3: TraitResolver key redesign not done) |
| 12: Lifetime elision + region inference | 2-3 weeks | P1 | Tasks 7, 9 | ⚠️ Partial (Task 7 DONE, Task 9 partial) |
| **13: `impl Drop` + RAII types** | **1 week** | **P1** | **Task 8** | **✅ Ready (Task 8 infrastructure COMPLETE)** |
| 14: Object safety + dyn Trait | 1 week | P2 | Task 3 | ⚠️ Blocked (Task 3 not done) |

### 2.1 Why Task 13 first?

1. **Infrastructure ready**: The Drop elaboration infrastructure
   (`ty_needs_drop`, `elaborate_drops`, drop glue codegen, driver
   integration) is complete from Stages 15.42-15.46.
2. **Lowest effort**: 1 week (vs 2-3 weeks for Tasks 11/12).
3. **Immediate user value**: RAII types (`File`, `Box<T>`, `MutexGuard`)
   are fundamental to systems programming.
4. **Unblocks Task 20**: `Box<T>` in prelude needs Drop support.

### 2.2 What Task 13 requires

The only missing piece is **parser support for `impl Drop for T`**:
1. Parse `impl Drop for Type { fn drop(&mut self) { body } }`.
2. Register the `Drop` impl in `TraitResolver` (so `is_drop_builtin` returns true).
3. The existing infrastructure takes over:
   - `ty_needs_drop` → returns `true` for the type.
   - `elaborate_drops` → inserts `Drop` terminators.
   - Drop glue codegen → calls `drop_adt_<N>`.
   - The user's `Drop::drop` method is called at scope end.

## 3. Task 13 Implementation Plan (Stages 15.55-15.59)

| Stage | Description | Effort | Status |
|-------|-------------|--------|--------|
| **15.55** | **Phase 3 design alignment** (this stage) | 0 (doc only) | ✅ DONE |
| 15.56 | Parser support for `impl Drop for T` | 2 days | ⏳ NEXT |
| 15.57 | TraitResolver registration + drop glue function emission | 2 days | ⏳ PLANNED |
| 15.58 | Conformance tests with `impl Drop` patterns | 2 days | ⏳ PLANNED |
| 15.59 | Gate review | 1 day | ⏳ PLANNED |

## 4. What's in scope for Task 13 MVP

- Parse `impl Drop for Type { fn drop(&mut self) { body } }`.
- Register in `TraitResolver` (so `is_drop_builtin` returns true).
- Emit `drop_adt_<N>` function that calls the user's `Drop::drop`.
- The user's `Drop::drop` method is called at scope end (via `elaborate_drops`).
- Conformance tests: `impl Drop` with side effects (e.g., counter increment).

## 5. What's NOT in scope for Task 13 MVP

- `Box<T>` in stdlib prelude (Task 20) — future.
- Drop order (reverse declaration) — future.
- Partial move handling — future.
- `ManuallyDrop<T>` — future.

## 6. Verification

- No code changes — design-only stage.
- All existing tests pass (zero regression).
