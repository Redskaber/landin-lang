# Stage 14.87 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.101.0 → v0.102.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.87 fixes 3 CRITICAL bugs found by the Round 3 independent audit
(general-purpose subagent). All 3 produced silent wrong output (§1.0
原則 5 violation) and were v0.1 blockers.

## 2. Bugs Fixed

### Bug A: Stage 14.86 match guard fix incomplete (overlap case)

**Symptom**: `match n { 0 if n == 0 => 100, 0 => 200, _ => 300 }` with
n=0 returned 200 (wrong — should return 100). The unguarded arm `0 => 200`
was added as a switch target, routing n=0 directly to the second arm and
bypassing the guarded first arm.

**Root cause**: `lower_match` built switch targets for unguarded arms
without considering whether preceding guarded arms had overlapping
patterns.

**Fix**: Track literal/Or/enum-variant values "claimed" by guarded arms
in `guarded_lit_values: Vec<ConstVal>`. When building switch targets for
unguarded arms, skip any value that was claimed. Those values fall through
to the otherwise block where the guarded arm runs first.

Additional sub-fixes:
- Extended Or-pattern handling in `build_pattern_equality_check`: each
  sub-pattern's failure now goes to the next sub-pattern's check (was: all
  went to the same next_block, causing only the first sub-pattern to be
  checked).
- Updated `was_claimed` logic in otherwise loop to handle all 3 pattern
  kinds (literal, Or-all-lit, enum-variant).
- Updated `has_guard || was_claimed` block to handle claimed-but-unguarded
  arms: pattern re-check without guard evaluation.

### Bug B: Tuple patterns with enum variant sub-patterns silently ignored

**Symptom**: `match t { (Opt::None, 0) => 0, ... }` — the enum variant
sub-pattern `Opt::None` was silently treated as a wildcard, causing
wrong match results.

**Root cause**: `build_tuple_pattern_condition` only checked literal
sub-patterns. Enum variant sub-patterns (`Opt::None`, `Opt::Some(x)`)
were skipped (treated as wildcards).

**Fix**: Added enum variant sub-pattern handling in
`build_tuple_pattern_condition`. For each enum variant sub-pattern at
index `i`:
1. Extract field `i` (the enum value) from the scrutinee tuple
2. Extract the enum's discriminant (field 0 of the enum struct)
3. Compare the discriminant to the variant index
4. If match, continue to next sub-pattern check; otherwise goto next_block

Uses `Ty::Adt(def_id, [])` for the field local type (was: `Error` → I32
fallback → GEP failure in codegen).

### Bug C: Explicit `self: &mut Type` form didn't propagate mutations

**Symptom**: `fn set(self: &mut Counter, v: i32) { self.value = v; }` —
calling `c.set(99)` didn't change `c.value`. The shorthand `&mut self`
form worked correctly.

**Root cause**: In `parse_params` (src/parser/generics.rs), when parsing
`self: &mut Type`, the parser read the explicit type but kept `self_kind`
as the default `Value(Immutable)`. The MIR lower then treated the receiver
as by-value instead of by-reference, causing mutations to not propagate.

**Fix**: After parsing the explicit type, check if it's `Ty::Ref`. If so,
update `self_kind` to `SelfKind::Ref(ref_mut)` to match the reference
mutability.

## 3. Verification

### Bug A verification

```rust
fn classify(n: i32) -> i32 {
    match n {
        0 if n == 0 => 100, 0 => 200, _ => 300,
    }
}
```
- classify(0) → 100 ✅ (was: 200)
- classify(1) → 300 ✅

Or-pattern: `match n { 0 | 1 if n == 0 => 100, 0 | 1 => 200, _ => 300 }`
- classify(0) → 100 ✅, classify(1) → 200 ✅, classify(5) → 300 ✅

Enum variant: `match s { Shape::Circle(r) if r > 0 => r*3, Shape::Circle(_) => 0, ... }`
- area(Circle(10)) → 30 ✅ (was: 0)
- area(Circle(-5)) → 0 ✅, area(Square(4)) → 16 ✅

### Bug B verification

```rust
match t { (Opt::None, 0) => 0, (Opt::None, _) => 1, (Opt::Some(_), 0) => 2, (Opt::Some(_), _) => 3 }
```
- describe((Some(10), 0)) → 2 ✅ (was: 0)
- describe((Some(10), 5)) → 3 ✅ (was: 1)
- describe((None, 0)) → 0 ✅, describe((None, 5)) → 1 ✅

### Bug C verification

```rust
fn set(self: &mut Counter, v: i32) { self.value = v; }
```
- c.get() → 10, c.set(99), c.get() → 99 ✅ (was: 10)

### Full test suite

- All 1951 rust tests pass (zero regression)
- All 5175 conformance tests pass (was 5172, +3 new run_ok tests:
  `e2e-runok-144-match-guard-overlap.lin`, `e2e-runok-145-tuple-enum-subpattern.lin`,
  `e2e-runok-146-explicit-self-mut.lin`)
- 0 clippy warnings, fmt clean

## 4. v0.1 Release Criteria — Still MET ✅

| Criterion | Status |
|-----------|--------|
| All P0 essential soundness gaps closed | ✅ All 3 new P0 bugs fixed |
| Documentation current | ✅ worklog, RELEASE_NOTES, gate-review current |
| Test suite passing | ✅ 1951 rust + 5175 conformance = 7126/7126 (100%) |
| Debug tooling available | ✅ 9 commands in `landin_debug.py` |
| API naming compliance | ✅ §23 audit clean |
| Process compliance | ✅ v3.22 stage-committee-process followed |
| Independent audit | ✅ Round 1 (14.84) + Round 2 (14.85) + Round 3 (14.87 — found 3 bugs, all fixed) |

## 5. Next Stage Plan

- **Stage 14.88**: Run Round 4 audit to verify all 3 Stage 14.87 fixes
  are correct and don't introduce new regressions.
- If Round 4 passes: v0.1 release is ready.
- If Round 4 finds issues: fix and run Round 5.
