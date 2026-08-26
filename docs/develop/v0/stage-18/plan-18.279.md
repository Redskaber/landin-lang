# Stage 18.279 — TD-LOC-CONTROL-FLOW Refactoring Plan

> **Author**: Super Z (main) — Stage Committee (ARCH-A + PM-A + REV-A)
> **Date**: 2026-08-25
> **Version**: v0.492.0 (no bump — refactoring)
> **Process**: stage-committee-process.md v7.3 §13.4 (重构即架构设计) + §13.4.1 (六大判据)

---

## 1. Architecture Analysis

### 1.1 Current State

`src/mir/lower/control_flow.rs` — 2301 LOC, handling 9 functions across 2 distinct responsibilities:

**Responsibility 1: Control flow lowering** (~843 LOC)
- `lower_short_circuit` (86 LOC) — And/Or short-circuit
- `lower_deref_expr` (22 LOC) — Deref expression
- `lower_block` (495 LOC) — Block lowering (let bindings, stmts, trailing expr)
- `lower_if` (61 LOC) — If/else lowering

**Responsibility 2: Match/Pattern lowering** (~1458 LOC)
- `lower_match` (922 LOC) — Match expression lowering
- `build_tuple_pattern_condition` (189 LOC) — Tuple pattern condition builder
- `build_pattern_equality_check` (313 LOC) — Pattern equality check
- `lower_nested_pattern_destructure` (57 LOC) — Nested pattern destructure
- `lower_nested_tuple_destructure` (69 LOC) — Nested tuple destructure

---

## 2. §13.4.1 Six Judgments (J1-J6)

| # | Judgment | Verification |
|---|----------|-------------|
| J1 | Architecture alignment | ✅ New `pattern_lower.rs` aligns with existing mir/lower/ module pattern |
| J2 | Single responsibility | ✅ After split: control_flow.rs handles control flow (block/if/short-circuit); pattern_lower.rs handles match/pattern lowering |
| J3 | One-way flow | ✅ lower_if/lower_block may call pattern functions (one direction); no back-calls |
| J4 | Compile-concept completeness | ✅ All pattern functions are self-contained |
| J5 | Stage division | ✅ Both files in mir/lower/ (same pipeline stage) |
| J6 | Reasonable size | ✅ After split: control_flow.rs ~843 LOC, pattern_lower.rs ~1458 LOC (near 1500 threshold but acceptable per J2) |

**All 6 judgments pass.**

---

## 3. Execution Steps

1. Create `src/mir/lower/pattern_lower.rs` with module doc
2. Move 5 pattern/match functions + their imports
3. Add `use super::pattern_lower::*` in control_flow.rs
4. Add `mod pattern_lower;` to mod.rs
5. Run cargo build + test + fmt + clippy
