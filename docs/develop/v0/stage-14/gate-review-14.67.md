# Stage 14.67 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.82.0 → v0.83.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.67 fixed a P0 bug in tuple pattern matching and added 3 audit-verified
patterns as run_ok tests (closures, bubble sort, stack). The tuple pattern bug
was a significant correctness issue affecting all tuple match expressions with
literal sub-patterns.

## 2. Bug Fixed

### Bug: Tuple match with literal sub-patterns always matched first arm

**Discovery**: Audit test `audit-stage14.67-tuple-match.lin` showed
`match p { (0, 0) => 0, (0, _) => 1, (_, 0) => 2, (a, b) => ... }` returned
`0` for ALL inputs.

**Root cause**: `lower_match` only handled top-level literal patterns
(`HirPatKind::Lit`) and Or-patterns as switch cases. Tuple patterns
(`HirPatKind::Tuple`) were treated as "non-literal" and fell through to the
otherwise block. The otherwise block found the first non-literal arm and
executed its body UNCONDITIONALLY — without checking if the tuple pattern
actually matched.

**Fix** (`src/mir/lower/control_flow.rs`): Added `build_tuple_pattern_condition`
helper that generates conditional checks for tuple patterns with literal
sub-patterns. For each literal sub-pattern at index `i`:
1. Extract field `i` from the scrutinee tuple
2. Compare it with the literal value (Eq)
3. Branch: if equal, continue to next check; if not, fall through to next arm

Wildcard (`_`) and Ident (binding) sub-patterns are skipped (always match).

The otherwise block now generates an if-else chain for tuple-pattern arms,
with non-tuple non-literal arms as catch-alls.

**Files changed**: `src/mir/lower/control_flow.rs` (lower_match otherwise
block rewritten + new `build_tuple_pattern_condition` helper)

## 3. Audit Patterns Tested (No Bugs Found)

The following patterns were tested and all work correctly:

| Pattern | Example | Status |
|---------|---------|--------|
| Closure with immutable capture | `make_adder(5)` = 15 | ✅ |
| Closure with no capture (fn ptr) | `apply_no_capture(inc, 10)` = 11 | ✅ |
| Inline closure in block | `use_inline_closure()` = 15 | ✅ |
| Multiple closures in sequence | `multi_closures()` = 21 | ✅ |
| Array of strings | `count_nonempty(["a","","b","c"])` = 3 | ✅ |
| Tuple match with literals+wildcards | `classify_pair` (5 cases) | ✅ (Bug fixed) |
| Match on array element | `classify_first` (3 cases) | ✅ |
| Find first zero (loop+break) | `find_first_zero` (2 cases) | ✅ |
| Stack (push/pop/peek/size) | Full LIFO operations | ✅ |
| Sum array | `sum_array([1,2,3,4,5])` = 15 | ✅ |
| Full bubble sort | `bubble_sort([5,3,1,4,2])` = [1,2,3,4,5] | ✅ |

## 4. Verification

- `cargo clean && cargo build --features llvm-backend` → ✅
- `cargo fmt` → ✅ (no changes)
- `cargo clippy --all-targets --features llvm-backend` → ✅ (0 warnings)
- `cargo test --features llvm-backend` → ✅ (1951 passed, 0 failed, 2 ignored)
- Conformance tests: 5149 (was 5145, +4 new run_ok)
- Pipeline coverage: 99.7% (686 paths, 684 verified)

## 5. D8 Review Dimensions

### D8.1 — Correctness
- Tuple pattern match fix addresses a real bug (verified by isolated test)
- Zero regression in existing 1951 rust tests + 5145 conformance tests
- New tests cover the exact pattern that was broken

### D8.2 — Architecture
- `build_tuple_pattern_condition` is a focused helper for tuple patterns
- The otherwise block now generates proper if-else chains
- Non-tuple non-literal arms (Wild, Ident) remain catch-alls

### D8.3 — API Naming
- `build_tuple_pattern_condition` follows `<verb>_<noun>_<noun>` pattern
- No public API changes (internal helper)

### D8.4 — Design-Driven Testing
- 4 new run_ok tests:
  - E-120: tuple match with literals (the bug)
  - E-121: closure with capture (audit-verified)
  - E-122: bubble sort (audit-verified)
  - E-123: stack (audit-verified)

### D8.5 — Long-term vs Short-term
- Tuple pattern match: long-term (proper conditional checks, not workarounds)
- The if-else chain approach scales to any number of arms

### D8.6 — Explicit vs Implicit
- Explicit check for `HirPatKind::Tuple` before generating conditions
- Explicit literal extraction from sub-patterns
- Wildcards and bindings explicitly skipped

### D8.7 — Errors vs Silent
- Bug was silent (wrong values, no compile error)
- Fix generates proper conditional checks (no silent miscompilation)

### D8.8 — General vs Special-case
- `build_tuple_pattern_condition` handles ALL tuple patterns with any
  combination of literal/wildcard/binding sub-patterns
- Not limited to specific tuple sizes or pattern combinations

## 6. Stage Outcome

**Stage 14.67 PASSED** — one P0 bug fixed (tuple pattern match), 3
audit-verified patterns added as run_ok tests, zero regression.

**Next steps** (priority order):
1. Continue auditing complex patterns (generics, trait dispatch, closures)
2. Address closure-to-FnPtr coercion (P1, identified in Stage 14.63)
3. Address remaining P0 blockers (GAP-4 lifetime elision, GAP-6 two-phase borrows)
4. Address deep soundness work (GAP-1 NLL, GAP-2 region inference, GAP-3 drop elaboration)
