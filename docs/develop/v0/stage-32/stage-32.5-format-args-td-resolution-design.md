# Stage 32.5 — TD-FORMAT-ARGS Resolution (Duplicate TD Cleanup)

> **Author**: PM-A + ARCH-A (Super Z)
> **Date**: 2026-09-01
> **Version**: v0.570.0 (target)
> **Stage**: v0.20 Stage 32.5
> **Predecessor**: v0.569.0 (Stage 32.3 + Stage 32.4 BLOCKED)
> **Tech-Debt Target**: TD-FORMAT-ARGS (P2, was BLOCKED v0.20+)

## §13.1 Design Alignment

Per §13.1 + §8.4.5: scanned `docs/lang-design/02-grammar.md` §4.4 (内建宏
清单 lists format! as supported). Scanned `docs/develop/v0/tech-debt-register.md`
TD-NO-FORMAT-MACRO (✅ Resolved Stage 18.186 + 18.202) and TD-FORMAT-VARIADIC
(✅ Resolved Stage 18.202).

## §1.2.1 Task Classification

L2 (documentation + minor code cleanup — TD duplicate resolution). L2 process
applies (skip §14.5 deep review, use §7.3 gate review instead).

## 5W2H — Root Cause Analysis

### WHAT
TD-FORMAT-ARGS (P2, "format! variadic args type handling not implemented",
BLOCKED v0.20+) is a **stale/duplicate TD**. The actual work it described was
already completed:

1. **TD-NO-FORMAT-MACRO** ✅ Resolved Stage 18.186 (format! macro MVP)
   + Stage 18.202 (variadic args).
2. **TD-FORMAT-VARIADIC** ✅ Resolved Stage 18.202 — `format!("x={}", x)`
   works via `lower_format_variadic_intrinsic` (598-LOC MIR walker).

TD-FORMAT-ARGS's description ("format! variadic args type handling not
implemented") is now factually wrong — the type handling IS implemented
(all args cast to i64, formatted via `__landin_i64_to_str`).

### WHY
The v0.19 Stage 31.8 audit incorrectly carried TD-FORMAT-ARGS forward as
"BLOCKED on v0.20+" without realizing it was already resolved by Stage 18.202.
The actual remaining work — migrating format! intrinsic to prelude impl — is
BLOCKED on v0.5+ method monomorphization (same root cause as
TD-VEC-PUSH-GET-MIGRATION, Stage 32.4).

### WHO
PM-A + ARCH-A (TD audit + cleanup).

### WHEN
Stage 32.5, after Stage 32.4 documented Vec::push/get migration blocker.

### WHERE
`docs/develop/v0/tech-debt-register.md` — TD-FORMAT-ARGS → RESOLVED;
add TD-FORMAT-MIGRATION (v0.5+ BLOCKED).

### HOW
1. Mark TD-FORMAT-ARGS as ✅ Resolved Stage 32.5 (duplicate of
   TD-NO-FORMAT-MACRO + TD-FORMAT-VARIADIC, both already resolved).
2. Add new TD-FORMAT-MIGRATION (P2, v0.5+ BLOCKED on method monomorphization)
   to properly track the actual remaining migration work.
3. Audit format! tests to ensure they all pass (no behavior change).
4. Update B1-B4 design writeback.

### HOW MUCH
0 LOC code changes. ~30 LOC documentation updates.
5095 tests (unchanged). 0 failures.

## §12 Solution Choice

Per §12 (最优 > 最小): the OPTIMAL solution here is honest TD bookkeeping,
not forcing a migration that's blocked on v0.5+ architecture.

Per §1.0 原则 4 (报错 > 静默): TD-FORMAT-ARGS's stale description was a silent
inaccuracy — fixing it makes the TD register honest.

Per §1.0 原则 9 (正确 > 妥协): don't pretend the migration is doable in v0.20
when it's blocked on the same v0.5+ method monomorphization as Vec::push/get.

## §14.5 D1-D8 (audit-only stage)

- D1 (fmt): clean ✅
- D2 (clippy): 0 warnings ✅
- D3 (build): success ✅
- D4 (lib tests): 898/898 ✅
- D5 (integration tests): 4197/4197 (4 ignored) ✅
- D6 (no P0/P1): TD-FORMAT-ARGS RESOLVED ✅
- D7 (architecture health): 9.85/10 (stable) ✅
- D8 (§1.6 终极检验): honest TD cleanup, not surface work ✅

## §14.8 Design Writeback (B1-B4)

### B1: TD Audit Match
- TD-FORMAT-ARGS description was stale → updated to RESOLVED.
- TD-FORMAT-MIGRATION (new) properly tracks v0.5+ migration blocker.

### B2: New TD Items
- TD-FORMAT-MIGRATION (P2, v0.5+ BLOCKED): migrate format! intrinsic
  (598 LOC) to prelude impl — blocked on method monomorphization
  (same as TD-VEC-PUSH-GET-MIGRATION).

### B3: Deviations
- None (audit-only stage, no code changes).

### B4: Architectural Limitations
- format! migration requires v0.5+ method monomorphization (per-instantiation
  fn body codegen with Param(N) substitution). Same root cause as
  TD-VEC-PUSH-GET-MIGRATION.

## Test Matrix

No new tests — this stage is audit-only. Existing format! tests
(`stage18_186_format_macro_tests.rs`) verify:
- `format!("literal")` works (8 positive tests).
- `format!("x={}", x)` works (3 positive tests, Stage 18.202).
- `format!("{}", 42)` works (Stage 18.202).
- `format!("a", "b")` works (Stage 18.202).

## §1.6 终极检验

> "这是针对根因的最优架构解，还是仅仅为了跑通测试的最小补丁？"

**Answer**: This is the **root-cause architectural fix** — honest TD bookkeeping.
The format! feature WORKS (Stage 18.186 + 18.202). What was broken was the
TD register's accuracy. Stage 32.5 fixes that.
