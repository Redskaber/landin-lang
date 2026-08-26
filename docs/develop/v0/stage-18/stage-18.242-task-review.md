# Stage 18.242 — Task Review: Fat Pointer Construction + Move Tracking Audit

> **Date**: 2026-08-23
> **Version**: v0.486.0 (no bump — audit + documentation)
> **Task ID**: stage18.242
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A + DEV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §17.6 + §17.8

## 1. 触发场景

Per Stage 18.240 v0.3 transition plan: Phase 1 Task 2 = Fat pointer construction.
Per Stage 18.241: str method resolution MVP done. Next: L2 (fat ptr) or L3/L4.

## 2. 依赖与基础设施完整能力审查

### 2.1 Fat Pointer Construction (L2) — DEFERRED

**Blocker**: Landin has no syntax for constructing a fat pointer `&str` from
two values (data pointer + length). Adding this requires parser + HIR + typeck
changes (~200 LOC). The ROI is low — only benefits String::as_str migration.

**Decision**: DEFER to v0.4+. as_str stays hardcoded (MVP, §17.6).

### 2.2 Expected-Type Propagation (L3) — DEFERRED

**Blocker**: Requires threading `expected_ty: Option<&Ty>` through ALL
`lower_expr_*` functions. This is a large architecture change (~500 LOC)
that affects the entire MIR lower pipeline.

**Decision**: DEFER to v0.3+ when expected-type infrastructure is designed
for trait solver + GATs. TD-TUPLE-CTOR-TYPECK stays deferred.

### 2.3 Move Tracking in Drop Elaboration (L4) — PARTIALLY EXISTS

**Current state**: `collect_moved_locals` (drop_elaboration.rs:74) already
tracks `Operand::Move` in rvalue assignments. It collects locals that have
been moved (ownership transferred) and skips their Drop terminators.

**Gaps identified**:
1. **Terminator moves not tracked**: `collect_moved_locals` only scans
   `StatementKind::Assign` rvalues. It does NOT scan `TerminatorKind::Call`
   args for `Operand::Move`. So if a local is moved into a function call,
   the move is not tracked → potential double-drop.
2. **StatementKind::Store moves not tracked**: `collect_moved_locals` doesn't
   handle `StatementKind::Store { val: Operand::Move(...), ... }`. If a
   value is moved through a Store (e.g., `*ptr = move(x)`), the move is
   not tracked.
3. **Box auto-drop**: Box::new allocates via `__landin_alloc` but the drop
   glue for Box doesn't call `__landin_dealloc`. Box users must manually
   dealloc. This is TD-BOX-AUTO-DROP.

**Per §17.8 (任务审查)**: L4 is partially implemented. The core move tracking
infrastructure exists (collect_moved_locals), but needs extension to cover
terminator + Store moves. This is feasible without large architecture changes.

## 3. Decision: Extend move tracking (L4) — feasible NOW

Per user directive "依赖与基础设施完整能力审查":
- L4 (move tracking) is partially implemented — the infrastructure exists
- Extending it to cover TerminatorKind::Call + StatementKind::Store is
  a localized change (~50-100 LOC) in `collect_moved_locals`
- This unblocks TD-DROP-MOVED-LOCALS (partial) and TD-BOX-AUTO-DROP (partial)

**Plan**: Extend `collect_moved_locals` to scan:
1. `TerminatorKind::Call` args for `Operand::Move`
2. `StatementKind::Store` val for `Operand::Move`
3. `Rvalue::Load` and `Rvalue::GetElementPtr` operands (already partially done)

## 4. No Code Changes This Stage

This is an audit stage. The implementation will be Stage 18.243.

## 5. Conclusion

- L2 (fat pointer construction): DEFERRED to v0.4+ (low ROI, complex parser change)
- L3 (expected-type propagation): DEFERRED to v0.3+ (large architecture change)
- L4 (move tracking): FEASIBLE NOW — infrastructure exists, needs extension

**Next**: Stage 18.243 — Extend `collect_moved_locals` to cover terminator + Store moves.
