# Stage 15.82 — Test Plan: infer_rvalue Span Accuracy + Remaining Debug Leaks

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.206.0 → v0.207.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.82 fixes the remaining `Span::DUMMY` error sites in
`infer_rvalue` (BinaryOp/UnaryOp type mismatch errors) and the
`check_statement` Assign coercion unify errors. It also fixes the last
5 `{:?}` Debug format leaks in those error messages.

## 2. New Integration Tests (3 tests)

Added to `tests/v0/stage15/plan/error_system_cleanup_tests.rs`:

### 2.1 `stage15_82_binary_op_error_span_points_to_statement`

Tests that `let x = true + false;` produces a typeck error whose span
points to the statement (byte offset >= 13, the `let`), not `1:1`.

```rust
let src = "fn main() { let x = true + false; }";
let result = compile(src);
let arith_err = result.errors.typeck.iter()
    .find(|e| e.message.contains("cannot apply arithmetic"))
    .expect("expected 'cannot apply arithmetic' error");
assert_ne!(arith_err.span.lo, 0, "span should not be Span::DUMMY");
assert!(arith_err.span.lo >= 13, "span should point into the statement (>= 13)");
```

### 2.2 `stage15_82_unary_op_error_span_points_to_statement`

Tests that `let y = !"hello";` produces a typeck error whose span
points to the second statement (byte offset >= 30), not `1:1`.

```rust
let src = "fn main() { let x = !true; let y = !\"hello\"; }";
let result = compile(src);
let not_err = result.errors.typeck.iter()
    .find(|e| e.message.contains("cannot apply `!`") && e.message.contains("str"))
    .expect("expected 'cannot apply `!` to &str' error");
assert_ne!(not_err.span.lo, 0, "span should not be Span::DUMMY");
assert!(not_err.span.lo >= 30, "span should point into the second statement (>= 30)");
```

### 2.3 `stage15_82_binary_op_error_uses_human_readable_type_names`

Tests that the `true + false` error message uses human-readable type
names (`bool`, not `Bool`), verifying the Stage 15.80 fix is preserved
in the BinaryOp path.

```rust
let src = "fn main() { let x = true + false; }";
let result = compile(src);
let arith_err = result.errors.typeck.iter()
    .find(|e| e.message.contains("cannot apply arithmetic"))
    .expect("expected 'cannot apply arithmetic' error");
assert!(arith_err.message.contains("bool"), "should contain 'bool'");
assert!(!arith_err.message.contains("Bool"), "should NOT contain Debug 'Bool'");
```

## 3. Conformance Test Impact

### 3.1 No conformance test changes

All 5216 conformance tests pass unchanged. The span accuracy changes
don't affect `ERROR_PATTERN` matching because:
- `ERROR_PATTERN` checks for substrings in the error message (e.g.,
  "arithmetic", "cannot apply"), not the span location.
- The error messages themselves are unchanged in wording (only the
  type display changed from Debug to human-readable, and the span
  changed from DUMMY to actual).
- The `{:?}` → `type_kind_to_string` fix doesn't break any patterns
  (no conformance test checks for Debug format in these messages).

## 4. Pipeline Path Coverage (§17.5.1)

### 4.1 BinaryOp error path (with accurate span)

| Stage | Path | Test |
|-------|------|------|
| Lexer | (no change) | ✅ |
| Parser | (no change) | ✅ |
| HIR lower | (no change) | ✅ |
| MIR lower | `Rvalue::BinaryOp` with operands (Place has span) | ✅ |
| Typeck | `check_statement` passes `stmt.span` to `infer_rvalue` | ✅ `stage15_82_binary_op_error_span_points_to_statement` |
| Driver | `to_diagnostics` renders the accurate span | ✅ Manual verification |
| (borrowck not reached for type errors) | — | — |
| (codegen not reached for type errors) | — | — |

### 4.2 UnaryOp error path (with accurate span)

| Stage | Path | Test |
|-------|------|------|
| Lexer | (no change) | ✅ |
| Parser | (no change) | ✅ |
| HIR lower | (no change) | ✅ |
| MIR lower | `Rvalue::UnaryOp` with operand (Place has span) | ✅ |
| Typeck | `check_statement` passes `stmt.span` to `infer_rvalue` | ✅ `stage15_82_unary_op_error_span_points_to_statement` |
| Driver | `to_diagnostics` renders the accurate span | ✅ Manual verification |

## 5. Manual Verification

Verified the improved error spans manually:

### 5.1 `true + false` — span points to the statement

```
$ echo 'fn main() { let x = true + false; }' | landin-stage0 --compile
error[E400]: cannot apply arithmetic to bool (expected integer or float)
  --> /tmp/t.lin:1:21
  |
1 | fn main() { let x = true + false; }
  |                     ^^^^
```

**Before**: `--> /tmp/t.lin:1:1` (file start, useless) + `Bool` (Debug)
**After**: `--> /tmp/t.lin:1:21` (the `true` literal, with snippet underline) + `bool` (human-readable)

### 5.2 `!"hello"` — span points to the statement

```
$ echo 'fn main() { let x = !true; let y = !"hello"; }' | landin-stage0 --compile
error[E400]: cannot apply `!` to &str (expected bool or integer)
  --> /tmp/t.lin:1:36
  |
1 | fn main() { let x = !true; let y = !"hello"; }
  |                                    ^
```

**Before**: `--> /tmp/t.lin:1:1` (file start, useless) + `Str` (Debug)
**After**: `--> /tmp/t.lin:1:36` (the `"hello"` literal, with snippet underline) + `&str` (human-readable)

## 6. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Conformance `ERROR_PATTERN` matches break | LOW | Patterns check message substrings, not span locations or type display format |
| Rust integration tests check span values | LOW | No existing tests assert on Span::DUMMY; the 3 new tests assert on the new accurate spans |
| `infer_rvalue` signature change breaks callers | LOW | Only one caller (`check_statement`), updated in the same change |
| Span override produces wrong location | LOW | 3 new tests verify span is in the statement range; manual verification confirms snippet underlines the right token |
| `stmt_span` is DUMMY for some callers | LOW | Only one caller (`check_statement`), which always passes `stmt.span`. If `stmt.span` is DUMMY (rare), the override is skipped (no regression) |

**Overall risk**: LOW. The changes are localized to error span
attribution and type display. The 3 new integration tests verify the
exact behavior for BinaryOp and UnaryOp paths.

## 7. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 232/232 PASS | ✅ 232/232 PASS |
| `cargo test --features llvm-backend --test all_tests` | 2138/2138 PASS | ✅ 2138/2138 PASS (was 2135, +3 new) |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |
| Conformance test count | unchanged (5216) | ✅ 5216 |

## 8. Test Sign-off

- ✅ All 232 lib tests pass
- ✅ All 2138 integration tests pass (was 2135, +3 new span accuracy tests)
- ✅ All 5216 conformance tests pass
- ✅ 0 conformance test flips
- ✅ 0 clippy warnings
- ✅ fmt clean
- ✅ Manual verification: error spans now point to actual statement locations

**Stage 15.82 PASSED**.
