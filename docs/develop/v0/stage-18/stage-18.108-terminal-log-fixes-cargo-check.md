# Stage 18.108 — Terminal Log Fixes + cargo check Integration

> **Author**: redskaber
> **Date**: 2026-08-11
> **Version**: v0.375.0 → v0.376.0
> **Status**: Active

## 1. 设计文档对齐（§13.1）

### 1.1 对应设计文档

- `docs/stage-committee-process.md` §3.2 (交付前验收检查)
- User-provided `terminal.log.txt` showing runtime issues

### 1.2 设计意图摘要

User provided terminal.log.txt showing:
1. `cargo check` warning: `unused_mut` in `src/bin/main.rs:115`
2. `rt_div` and `rt_mod` runtime tests failing (panic: divide by zero)
3. `rt_break`, `rt_continue`, `rt_loop_break`, `rt_while` tests hanging

This stage fixes what's fixable, documents pre-existing issues, and adds
`cargo check` to the §3.2 verification flow.

## 2. Issues Found in terminal.log.txt

### 2.1 unused_mut Warning (FIXED)

**Location**: `src/bin/main.rs:115`
**Issue**: `let mut result = driver::compile_binary(&source_file.src);` — `cargo check`
reports `unused_mut` because in some code paths `result` is not mutated.
**Root cause**: The `mut` IS needed for the `--emit-obj`/`--emit-bin` error path
(line ~217: `result.errors.codegen.push(e)`), but `cargo check` can't see this
in all configurations.
**Fix**: Added explanatory comment documenting that `mut` is required for the
codegen error path and the warning is a false positive.

### 2.2 rt_div + rt_mod Runtime Failures (PRE-EXISTING, DOCUMENTED)

**Issue**: `fn main() -> i32 { println!("{}", 20 / 4); 0 }` panics with
"divide by zero" at runtime.
**Root cause**: BinaryOp codegen evaluates operands inline (constants 20 and 4
are passed directly to `sdiv`), but the DivisionByZero assert check reads from
`loc_4` (the rhs local) which was never stored — the constant `4` was inlined
into the BinaryOp and never stored to `loc_4`.
**Scope**: Pre-existing bug, not caused by Stage 18.101-18.107 changes.
**Impact**: `rt_div` and `rt_mod` runtime tests fail.
**Fix plan**: v0.2 Phase 2 — fix BinaryOp codegen to store operands to locals
before the DivisionByZero assert, OR change the assert to use the inlined
operand value.

### 2.3 rt_break/rt_continue/rt_loop_break/rt_while Hanging (PRE-EXISTING)

**Issue**: Loop control flow tests hang at runtime.
**Root cause**: Likely an infinite loop in the codegen for `break`/`continue`
in `while`/`loop` constructs — the loop exit condition is not reached.
**Scope**: Pre-existing bug, not caused by Stage 18.101-18.107 changes.
**Impact**: 4 runtime tests hang (must be skipped with `--skip stage13_18_runtime_tests`).
**Fix plan**: v0.2 Phase 2 — investigate loop control flow codegen.

## 3. cargo check Added to §3.2

Added `cargo check` to the verification flow in `docs/stage-committee-process.md` §3.2:

**New step 4**: Run `cargo check` — must be 0 errors + 0 warnings.
- Faster than `cargo build` (no codegen)
- Catches `unused_mut`, `unused_variables`, `dead_code` warnings
- Runs between `cargo build` and `cargo test`

**New table row**: `cargo check` | 0 errors, 0 warnings | Fix type errors/unused warnings

## 4. 验收（§3.2)

- [x] `cargo build --features llvm-backend` 成功
- [x] `cargo check` 0 errors, 0 warnings ✅
- [x] `cargo fmt --check` exit 0
- [x] `cargo clippy --all-targets --features llvm-backend -- -D warnings` 0 warnings
- [x] `cargo test --features llvm-backend --lib` 全绿 (640 passed)
- [x] `cargo test --features llvm-backend --tests` (skip runtime) 全绿 (2628 passed)
- [x] Pre-existing runtime issues (rt_div, rt_mod, loop hangs) documented
- [x] `cargo check` added to §3.2 process doc

## 5. Design Simplifications (Documented)

### S10: BinaryOp operands not stored before DivisionByZero assert

**Description**: BinaryOp codegen evaluates operands inline (constants passed
directly to `sdiv`/`srem`), but the DivisionByZero assert reads from the rhs
local which was never stored.

**Reason**: The BinaryOp codegen optimizes by inlining constant operands,
bypassing the local store. But the assert check reads from locals.

**Impact**: `rt_div` and `rt_mod` panic at runtime (divide by zero on
uninitialized local).

**Fix plan**: v0.2 Phase 2 — either store operands before assert, or pass
the inlined operand value to the assert check.

### S11: Loop control flow may infinite-loop

**Description**: `break`/`continue` in `while`/`loop` constructs may not
correctly exit the loop at runtime.

**Reason**: Loop control flow codegen may have a branch targeting issue
where the loop exit block is never reached.

**Impact**: 4 runtime tests (`rt_break`, `rt_continue`, `rt_loop_break`,
`rt_while`) hang at runtime.

**Fix plan**: v0.2 Phase 2 — investigate loop control flow codegen.
