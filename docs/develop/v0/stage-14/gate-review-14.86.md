# Stage 14.86 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.100.0 → v0.101.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.86 fixes a silent correctness bug in match arms with guards.
The HIR stored `arm.guard: Option<HirExpr>` but the MIR lower ignored
guards entirely — guarded arms with literal patterns were added as direct
switch targets, matching without checking the guard condition.

## 2. Bug Fixed

### Match arms with guards silently ignored the guard

**Symptom**: `match n { 0 => 0, x if x < 10 => 1, _ => 2 }` with n=100
returned 1 (wrong — should return 2). The guard `x < 10` was never
evaluated; the arm matched any value because the literal-pattern switch
target was added regardless of guard presence.

**Root cause**: In `src/mir/lower/control_flow.rs::lower_match`, the
literal-pattern handling at line 820-828 pushed `(ConstVal::Int(n),
arm_block)` to `targets` without checking `arm.guard`. The Or-pattern
and enum-variant handling had the same bug.

**Fix** (3 changes in `lower_match` + 1 new helper):

1. **Skip switch targets for guarded arms**: Added `let has_guard =
   arm.guard.is_some();` check. Skip the literal/Or/enum target push
   when `has_guard` is true. These arms will be handled in the
   otherwise block where we can evaluate both pattern AND guard.

2. **Handle guarded arms in otherwise block**: New code path that:
   - Binds pattern variables (for Ident patterns) BEFORE evaluating
     guard, so the guard can reference them
   - Calls `build_pattern_equality_check` for literal/Or/enum patterns
     to re-check the pattern match (since they weren't added as switch
     targets)
   - Evaluates the guard expression
   - If both pattern and guard pass, executes arm body; otherwise falls
     through to next arm

3. **New helper `build_pattern_equality_check`**: Generates the
   pattern-match check for guarded arms:
   - `HirPatKind::Lit(lit_expr)` → `scrut == lit` (bool switch on result)
   - `HirPatKind::Or(sub_pats)` → `scrut == lit1 || scrut == lit2 || ...`
     chain (each sub-pattern checked separately, going to match_block
     on first match)
   - `HirPatKind::Path/TupleStruct/Struct` (enum variant) → extracts
     discriminant (field 0 of scrut struct) and checks
     `discr == variant_idx`

## 3. Verification

### Test cases verified

```rust
fn classify(n: i32) -> i32 {
    match n {
        0 => 0,
        x if x < 0 => -1,
        x if x < 10 => 1,
        _ => 2,
    }
}
```

| Input | Expected | Actual | Status |
|-------|----------|--------|--------|
| classify(0) | 0 | 0 | ✅ |
| classify(-5) | -1 | -1 | ✅ |
| classify(5) | 1 | 1 | ✅ |
| classify(100) | 2 | 2 | ✅ |

```rust
fn classify_with_ident(n: i32) -> i32 {
    match n {
        0 => 0,
        x if x < 10 => x,  // returns the bound value
        _ => 100,
    }
}
```

| Input | Expected | Actual | Status |
|-------|----------|--------|--------|
| classify_with_ident(5) | 5 | 5 | ✅ (returns bound x) |
| classify_with_ident(20) | 100 | 100 | ✅ (guard fails, falls to _) |

### Full test suite

- All 1951 rust tests pass (zero regression)
- All 5172 conformance tests pass (was 5171, +1 new run_ok test:
  `e2e-runok-143-match-guard.lin`)
- 0 clippy warnings, fmt clean

## 4. P0/P1 Status Update

| Gap | Severity | Status |
|-----|----------|--------|
| GAP-1 | P0 | ✅ FIXED (Stage 14.81) |
| GAP-2 | P0 | Deferred (L3) |
| GAP-3 | P0 | Deferred (L3) |
| GAP-4 | P0 | Deferred (L2) |
| GAP-5 | P0 | ✅ Working (Stage 14.81) |
| GAP-6 | P0 | ✅ Working (Stage 14.81) |
| GAP-7 | P1 | ✅ Partial (Stage 14.82 + 14.84) |
| **NEW** | **P0** | **✅ FIXED (Stage 14.86)** — match guards silently ignored |
| GAP-9 | P0 | Deferred (L3) |
| GAP-14 | P1 | Pending (L2) |
| GAP-15 | P1 | Deferred (L3) |

## 5. v0.1 Release Criteria — Still MET ✅

| Criterion | Status |
|-----------|--------|
| All P0 essential soundness gaps closed | ✅ GAP-1/5/6 + new match-guard bug all fixed |
| Documentation current | ✅ README, RELEASE_NOTES, worklog current |
| Test suite passing | ✅ 1951 rust + 5172 conformance = 7123/7123 (100%) |
| Debug tooling available | ✅ 9 commands in `landin_debug.py` |
| API naming compliance | ✅ §23 audit clean |
| Process compliance | ✅ v3.22 stage-committee-process followed |
| Independent audit | ✅ Round 1 (Stage 14.84) + Round 2 (Stage 14.85) both PASS |

## 6. Next Stage Plan

- **Stage 14.87**: Run Round 3 audit (agent groups validation) to verify
  Stage 14.86 match guard fix is correct + doesn't introduce regressions.
- **Stage 14.88+**: Tackle GAP-14 (cross-module visibility) or other P1 items
  if user wants more P0/P1 fixes before final v0.1 release.
