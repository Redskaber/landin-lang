# Stage 15.42 — Drop Elaboration Design Doc (Design Alignment)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.167.0 → v0.168.0
> **Process**: stage-committee-process.md v3.23 §13.4 (设计对齐) + §29
> **v0.2 Phase 2 Task 8 (step 1 of 6)**: Wire up drop elaboration (HP-12)
> **Design doc**: `docs/lang-design/25-drop-elaboration.md`

## 1. Executive Summary

Stage 15.42 is a **design-only stage** — no code changes. It creates the
design document for Drop elaboration (Task 8, HP-12), which is the next
v0.2 Phase 2 task after the NLL migration (Task 7) was completed in
Stage 15.41.

Per §13.4 (设计对齐 — design before implementation): the design doc must
exist before any implementation work begins. This stage creates the doc
and plans the implementation stages (15.43-15.47).

Per §1.0 原則 1 "长期 > 短期": investing in design upfront prevents
costly rework during implementation.

## 2. What Was Done

### 2.1 Created `docs/lang-design/25-drop-elaboration.md`

The design document covers:
1. **Problem statement**: No user-defined `Drop` support; `TerminatorKind::Drop`
   is a no-op; no `drop_elaboration` module.
2. **Design**: `needs_drop` analysis, drop insertion pass, drop glue codegen,
   drop order (fields in declaration order, locals in reverse).
3. **Migration strategy**: 6 stages (15.42 design + 15.43-15.46
   implementation + 15.47 review).
4. **Dependencies**: Task 7 (NLL) — COMPLETE; Task 1 (Ty interning) — COMPLETE;
   TraitResolver — EXISTS.
5. **Testing strategy**: unit tests, integration tests, conformance tests.
6. **API naming compliance**: all new symbols follow §23 conventions.
7. **Open questions**: field type traversal, block splitting, drop glue
   naming, interaction with `move`.

### 2.2 Reviewed existing infrastructure

- `TerminatorKind::Drop { place, target, replace }` exists in MIR
  (`src/mir/body.rs`).
- `TraitResolver::is_drop_builtin(def_id, interner)` exists
  (`src/traits/resolver.rs`).
- `StatementKind::StorageDead(LocalId)` is emitted at function return
  (`src/mir/lower/mod.rs`).
- Codegen for `TerminatorKind::Drop` is a no-op with a TODO comment
  (`src/codegen/terminator.rs` line 422).
- The `drop_elaboration` module was removed in Stage 14.105 (dead code).

## 3. Implementation Plan (Stages 15.42-15.47)

| Stage | Description | Effort | Status |
|-------|-------------|--------|--------|
| **15.42** | **Design doc** (this stage) | 0 (doc only) | ✅ DONE |
| 15.43 | Implement `ty_needs_drop` analysis + unit tests | 0.5 day | ⏳ NEXT |
| 15.44 | Implement `elaborate_drops` pass (insert `Drop` terminators) + tests | 1 day | ⏳ PLANNED |
| 15.45 | Implement drop glue codegen + `TerminatorKind::Drop` codegen | 1 day | ⏳ PLANNED |
| 15.46 | Integration: wire into driver, add conformance tests | 0.5 day | ⏳ PLANNED |
| 15.47 | Gate review + deep review | 0.5 day | ⏳ PLANNED |

Total estimated effort: 3-5 days (per v0.2-preparation.md).

## 4. Verification

- No code changes — `cargo build` / `cargo test` pass unchanged.
- The design doc is the artifact.

## 5. Stage Gate Review (self-review per §9.3)

| Check | Status |
|-------|--------|
| Design doc exists (`docs/lang-design/25-drop-elaboration.md`) | ✅ |
| Design covers problem, solution, migration, testing | ✅ |
| API naming compliance (§23) planned | ✅ |
| Dependencies verified (Task 7 + Task 1 + TraitResolver) | ✅ |
| Open questions documented | ✅ |
| No code changes (design-only stage) | ✅ |
| All existing tests pass (zero regression) | ✅ |

## 6. Conclusion

Stage 15.42 creates the design foundation for Drop elaboration. The design
doc covers the full scope: `needs_drop` analysis, drop insertion, drop glue
codegen, and the staged implementation plan.

The next stage (15.43) will implement `ty_needs_drop` — the analysis that
determines whether a type needs drop glue. This is the foundation for the
drop insertion pass (Stage 15.44) and drop glue codegen (Stage 15.45).
