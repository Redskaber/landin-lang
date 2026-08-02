# Stage 15.53 — Closure Redesign Design Doc (Task 10 Design Alignment)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.178.0 → v0.179.0
> **Process**: stage-committee-process.md v3.23 §13.4 (设计对齐) + §29
> **v0.2 Phase 2 Task 10 (step 1 of 6)**: Synthesized `call` function per closure (HP-3)
> **Design doc**: `docs/lang-design/27-closure-redesign.md`

## 1. Executive Summary

Stage 15.53 is a **design-only stage** — no code changes. It creates the
design document for closure redesign (Task 10, HP-3), which is the final
v0.2 Phase 2 task.

## 2. What Was Done

### 2.1 Created `docs/lang-design/27-closure-redesign.md`

The design document covers:
1. Problem statement: inline approach (Stage 13.3a) has limitations (no recursion, no Fn/FnMut/FnOnce, code duplication).
2. Design: Strategy A — synthesize a separate `call` function per closure.
3. What's already implemented: closure capture, inline call, closure struct type.
4. What needs to be implemented: synthesized `call` function, fat pointer representation, call codegen, Fn/FnMut/FnOnce traits.
5. Migration strategy: retain inline as fallback during migration.
6. Dependencies: Task 1 (Ty interning) — COMPLETE; closure capture — EXISTS.
7. Open questions: capture layout, by-value vs by-reference, drop interaction, region interaction.

### 2.2 Reviewed existing closure infrastructure

- `closure_capture.rs` — extracts captures from the environment.
- `expr_operand.rs::inline_closure_call` — inlines the body at the call site.
- `TyKind::Closure(def_id, substs)` — closure type in MIR.

## 3. Implementation Plan (Stages 15.53-15.58)

| Stage | Description | Effort | Status |
|-------|-------------|--------|--------|
| **15.53** | **Design doc** (this stage) | 0 (doc only) | ✅ DONE |
| 15.54 | Synthesize `call` function per closure | 1 week | ⏳ NEXT |
| 15.55 | Closure value as fat pointer + call codegen | 3 days | ⏳ PLANNED |
| 15.56 | Fn/FnMut/FnOnce trait impls | 3 days | ⏳ PLANNED |
| 15.57 | Conformance tests | 2 days | ⏳ PLANNED |
| 15.58 | Gate review | 0.5 day | ⏳ PLANNED |

## 4. Verification

- No code changes — design-only stage.
- All existing tests pass (zero regression).

## 5. Stage Gate Review

| Check | Status |
|-------|--------|
| Design doc exists | ✅ |
| Design covers problem, solution, migration, testing | ✅ |
| API naming compliance (§23) planned | ✅ |
| Dependencies verified | ✅ |
| Open questions documented | ✅ |
| No code changes (design-only stage) | ✅ |
| All existing tests pass (zero regression) | ✅ |
