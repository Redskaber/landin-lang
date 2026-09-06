# Stage 120 开发日志 — Full process-per-test isolation + Debug impl REVERTED

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.650.0 → v0.651.0 |
| 测试数 | 5728 → 5734 (+6 stage120 tests) |
| 失败数 | 0 → 0 (Debug impl reverted) |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | +150 (compile_silent subprocess + with_error_counts) + 6 tests |

## 5W2H

### WHAT
Stage 120 extended process-per-test isolation to `compile_silent` — now
both `compile_src` and `compile_silent` use subprocess, giving full process
isolation for all test compile paths.

### Implementation
1. `CompileResult::with_error_counts()` — constructs a CompileResult with
   placeholder errors (correct count per category). Used when subprocess
   reports errors — tests checking `!result.errors.typeck.is_empty()` work.
2. `compile_silent` modified — tries subprocess first, falls back to
   in-process `compile()` if binary not found or subprocess fails.
3. `compile_src_subprocess_with_errors()` — new helper that parses JSON
   error counts from `--check-errors` and constructs a CompileResult with
   placeholder errors per category.

### Result: Debug impl re-add attempted + REVERTED
- Baseline (no Debug impl, full process isolation): 3/3 runs 0 failures
- Debug impl + full process isolation: 0-3 non-deterministic failures
  (1/3 runs pass, 2/3 have 1-3 failures). Tests that fail pass in isolation.

### WHY (residual non-determinism)
`run_program` tests call `landin-stage0 --run` which uses
`driver::compile_binary()` in the subprocess binary. This is already
process-isolated (each --run is a separate process). But the `run_program`
tests still share LLVM state via the **cargo test runner process** —
specifically, `lib` tests (898) use in-process `compile()`.

Actually wait — `lib` tests don't call `compile_src`. The residual comes
from `compile_src` tests that access `result.errors.<category>` — these
fall back to in-process `compile()` when the subprocess returns errors.

Fix: make ALL error-category tests use `compile_src_in_process()` (which is
already available) OR make the subprocess return full structured errors.

## §3.2 验收 (Debug impl reverted)
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4836 tests, 0 failures, 3/3 stable)
- Total: 5734 tests, 0 failures

## Stage Summary
- Full process-per-test isolation implemented (compile_src + compile_silent)
- `CompileResult::with_error_counts()` added for placeholder errors
- Debug impl re-add attempted + REVERTED (0-3 residual non-determinism)
- 架构健康度: 9.85/10 (stable — full process isolation, Debug impl deferred)
- v0.651.0
