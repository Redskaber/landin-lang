# Stage 14.68 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.83.0 → v0.84.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.68 fixed two P0 bugs: a parser issue where `while {} -1` was parsed as
binary subtraction, and a control-flow issue where loop body `return` was
overwritten by `Goto`. Both bugs were silent — compilation failed with
confusing type errors.

## 2. Bugs Fixed

### Bug 1: `while {} -1` parsed as binary subtraction

**Discovery**: Audit test `audit-stage14.68-while-return.lin` failed with
"cannot apply arithmetic to Tuple([])" for `while i < 5 { return i; } -1`.

**Root cause**: The parser's binary operator parsers (parse_add_expr, parse_cmp_expr,
etc.) greedily consumed binary operators after ANY expression, including block-like
expressions. `while {} -1` was parsed as `(while_result) - 1` instead of two
statements: `while {}` and `-1`.

Stage 14.63 fixed this for postfix operators (Call/Index) but NOT for binary operators.

**Fix** (`src/parser/expr.rs`): Added `is_block_like_expr(&lhs)` check at the start
of EVERY binary operator parser (parse_or_expr through parse_add_expr). If LHS is
block-like, return immediately without consuming any binary operators.

### Bug 2: While/Loop body with return overwrote Return terminator

**Discovery**: Same test — after the parser fix, the `return i` inside the while
body was being overwritten by `Goto(cond_block)`.

**Root cause**: Loop lowering called `cx.terminate(Goto(...))` unconditionally after
`lower_block`, even if the body already terminated (via `return`, `break`, `continue`).

**Fix** (`src/mir/lower/expr_operand.rs`): Added `if !cx.is_terminated()` check before
`cx.terminate(Goto(...))` in While, For, and Loop lowering.

## 3. Audit Patterns Tested (No Bugs Found)

| Pattern | Example | Status |
|---------|---------|--------|
| String comparison | `cmp_str("hello", "hello")` = 0 | ✅ |
| Person &mut self birthday | `p.birthday()` = 31 | ✅ |
| Nested struct mutation | `o.bump()` = 42 | ✅ |
| Array of structs + methods | `sum_points` = 21 | ✅ |
| Enum+struct payload | `Shape::Point(Point{5,6})` = 30 | ✅ |
| While+early return | `find_first` = 2/-1 | ✅ (Bug 1+2 fixed) |
| Min/max tuple | `min_max([3,1,4,1,5])` = (1,5) | ✅ |
| Complex enum dispatch | `Expr::Num/Add/Mul/Neg` | ✅ |
| Deep nesting (4 levels) | `deep_nesting` 5 cases | ✅ |
| Fibonacci iterative | `fib_iter(10)` = 55 | ✅ |
| Prime check | `is_prime(7)` = true | ✅ |

## 4. Verification

- `cargo clean && cargo build --features llvm-backend` → ✅
- `cargo fmt` → ✅ (no changes)
- `cargo clippy --all-targets --features llvm-backend` → ✅ (0 warnings)
- `cargo test --features llvm-backend` → ✅ (1951 passed, 0 failed, 2 ignored)
- Conformance tests: 5153 (was 5149, +4 new run_ok)
- Pipeline coverage: 99.7% (690 paths, 688 verified)

## 5. Stage Outcome

**Stage 14.68 PASSED** — two P0 bugs fixed, zero regression, 4 new run_ok tests.

**Next steps** (priority order):
1. Continue auditing complex patterns (generics, trait dispatch, closures)
2. Address GAP-6 (two-phase borrows — &mut self calling &mut self)
3. Address remaining P0 blockers (GAP-4 lifetime elision)
4. Address deep soundness work (GAP-1 NLL, GAP-2 region inference, GAP-3 drop elaboration)
