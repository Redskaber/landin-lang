# Stage 15.81 — Test Plan: Typeck Error Span Accuracy Fix

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.205.0 → v0.206.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.81 fixes error span accuracy in `src/typeck/checker.rs`. 7
typeck error sites now use actual source spans (from `Operand::Place`
or `Terminator::span`) instead of `Span::DUMMY`. Also fixes the last
`{:?}` Debug format leak in the SwitchInt error message.

## 2. New Integration Tests (3 tests)

Added to `tests/v0/stage15/plan/error_system_cleanup_tests.rs`:

### 2.1 `stage15_81_if_condition_span_points_to_condition`

Tests that `if 42 { 1 }` produces a typeck error whose span points to
the `42` literal (byte offset 15), not `1:1` (file start).

```rust
let src = "fn main() { if 42 { 1 } }";
let result = compile(src);
let mismatch_err = result.errors.typeck.iter()
    .find(|e| e.message.contains("mismatched types"))
    .expect("expected mismatched types error");
assert_ne!(mismatch_err.span.lo, 0, "span should not be Span::DUMMY");
assert_eq!(mismatch_err.span.lo, 15, "span should point to `42` at byte 15");
```

### 2.2 `stage15_81_call_non_function_span_points_to_callee`

Tests that `let x = 42; x();` produces a typeck error whose span
points to the `x` in `x()` (byte offset 24), not `1:1`.

```rust
let src = "fn main() { let x = 42; x(); }";
let result = compile(src);
let call_err = result.errors.typeck.iter()
    .find(|e| e.message.contains("expected function"))
    .expect("expected 'expected function' error");
assert_ne!(call_err.span.lo, 0, "span should not be Span::DUMMY");
assert_eq!(call_err.span.lo, 24, "span should point to `x` in `x()` at byte 24");
```

### 2.3 `stage15_81_error_uses_human_readable_type_names`

Tests that the `if 42` error message uses human-readable type names
(`bool`, not `Bool`), verifying the Stage 15.80 fix is preserved.

```rust
let src = "fn main() { if 42 { 1 } }";
let result = compile(src);
let mismatch_err = result.errors.typeck.iter()
    .find(|e| e.message.contains("mismatched types"))
    .expect("expected mismatched types error");
assert!(mismatch_err.message.contains("bool"), "should contain 'bool'");
assert!(!mismatch_err.message.contains("Bool"), "should NOT contain Debug 'Bool'");
assert!(!mismatch_err.message.contains("Infer("), "should NOT contain Debug 'Infer('");
```

## 3. Conformance Test Impact

### 3.1 No conformance test changes

All 5216 conformance tests pass unchanged. The span accuracy changes
don't affect `ERROR_PATTERN` matching because:
- `ERROR_PATTERN` checks for substrings in the error message (e.g.,
  "immutable", "cannot borrow"), not the span location.
- The error messages themselves are unchanged (only the span is fixed).
- The `{:?}` → `type_kind_to_string` fix in SwitchInt doesn't break
  any patterns (no conformance test checks for Debug format).

## 4. Pipeline Path Coverage (§17.5.1)

### 4.1 SwitchInt error path (with accurate span)

| Stage | Path | Test |
|-------|------|------|
| Lexer | (no change) | ✅ |
| Parser | (no change) | ✅ |
| HIR lower | (no change) | ✅ |
| MIR lower | `SwitchInt` terminator with `discr` operand (Place has span) | ✅ |
| Typeck | `check_terminator` SwitchInt arm uses `operand_span(discr)` | ✅ `stage15_81_if_condition_span_points_to_condition` |
| Driver | `to_diagnostics` renders the accurate span | ✅ Manual verification |
| (borrowck not reached for type errors) | — | — |
| (codegen not reached for type errors) | — | — |

### 4.2 Call error path (with accurate span)

| Stage | Path | Test |
|-------|------|------|
| Lexer | (no change) | ✅ |
| Parser | (no change) | ✅ |
| HIR lower | (no change) | ✅ |
| MIR lower | `Call` terminator with `func` operand (Place has span) | ✅ |
| Typeck | `check_terminator` Call arm uses `operand_span(func)` + `term.span` | ✅ `stage15_81_call_non_function_span_points_to_callee` |
| Driver | `to_diagnostics` renders the accurate span | ✅ Manual verification |

## 5. Manual Verification

Verified the improved error spans manually:

### 5.1 `if 42` — span points to `42`

```
$ echo 'fn main() { if 42 { 1 } }' | landin-stage0 --compile
error[E400]: mismatched types: expected {integer}, found bool
  --> /tmp/t.lin:1:16
  |
1 | fn main() { if 42 { 1 } }
  |                ^^

note: expected: {integer}
  --> /tmp/t.lin:1:16
  |
1 | fn main() { if 42 { 1 } }
  |                ^^

note: found: bool
  --> /tmp/t.lin:1:16
  |
1 | fn main() { if 42 { 1 } }
  |                ^^
```

**Before**: `--> /tmp/t.lin:1:1` (file start, useless)
**After**: `--> /tmp/t.lin:1:16` (the `42` literal, with snippet underline)

### 5.2 `x()` — span points to `x`

```
$ echo 'fn main() { let x = 42; x(); }' | landin-stage0 --compile
error[E400]: expected function, found i32
  --> /tmp/t2.lin:1:25
  |
1 | fn main() { let x = 42; x(); }
  |                         ^
```

**Before**: `--> /tmp/t2.lin:1:1` (file start, useless)
**After**: `--> /tmp/t2.lin:1:25` (the `x` in `x()`, with snippet underline)

## 6. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Conformance `ERROR_PATTERN` matches break | LOW | Patterns check message substrings, not span locations |
| Rust integration tests check span values | LOW | No existing tests assert on Span::DUMMY; the 3 new tests assert on the new accurate spans |
| Span override produces wrong location | LOW | 3 new tests verify exact byte offsets; manual verification confirms snippet underlines the right token |
| `operand_span` returns DUMMY for Constants | LOW | Constants are rare in error positions; fall back to DUMMY is acceptable (no regression — same as before) |
| Unify error span override leaks | LOW | Override happens before `self.errors.push(*e)`; no other consumer sees the original DUMMY |

**Overall risk**: LOW. The changes are localized to error span
attribution. The 3 new integration tests verify the exact byte offsets
for the two most common error paths (SwitchInt + Call).

## 7. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 232/232 PASS | ✅ 232/232 PASS |
| `cargo test --features llvm-backend --test all_tests` | 2135/2135 PASS | ✅ 2135/2135 PASS (was 2132, +3 new) |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |
| Conformance test count | unchanged (5216) | ✅ 5216 |

## 8. Test Sign-off

- ✅ All 232 lib tests pass
- ✅ All 2135 integration tests pass (was 2132, +3 new span accuracy tests)
- ✅ All 5216 conformance tests pass
- ✅ 0 conformance test flips
- ✅ 0 clippy warnings
- ✅ fmt clean
- ✅ Manual verification: error spans now point to actual source locations

**Stage 15.81 PASSED**.
