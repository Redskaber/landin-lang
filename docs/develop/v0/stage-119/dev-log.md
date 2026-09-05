# Stage 119 开发日志 — Process-per-test isolation implemented + Debug impl REVERTED

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.649.0 → v0.650.0 |
| 测试数 | 5723 → 5728 (+5 stage119 tests) |
| 失败数 | 0 → 0 (Debug impl reverted) |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | +100 (subprocess compile_src) + 0 (Debug impl reverted) + 5 tests |

## 5W2H

### WHAT
Stage 119 implemented process-per-test isolation: `compile_src` now uses
subprocess (`landin-stage0 --check-errors`) for error-free compilations,
giving each test fresh LLVM C++ state.

### Implementation
1. **`src/bin/main.rs`**: Added `--check-errors` flag that outputs error
   counts as JSON to stdout, then exits.
2. **`src/driver/mod.rs`**: Added `CompileResult::empty_result()` constructor.
3. **`tests/common/mod.rs`**: `compile_src` now tries subprocess first
   (returns `CompileResult::empty_result()` for error-free compilations).
   Falls back to in-process `compile()` for error cases (structured errors
   needed by tests accessing `result.errors.typeck` etc.).

### Result: Debug impl re-add attempted + REVERTED
- Baseline (no Debug impl, with subprocess): 3/3 runs 0 failures (stable)
- Debug impl + subprocess: 0-2 non-deterministic failures per run
  (2/6 runs pass, 4/6 have 1-2 failures). Tests that fail pass in isolation.

### WHY (residual non-determinism)
The subprocess path only handles `compile_src` (error-free compilations).
194 tests still use in-process `compile()` directly (for structured error
access). These share LLVM C++ state → residual non-determinism (0-2 failures).
Fix: make `compile_silent` also use subprocess (Stage 120+).

### §3.2 验收 (Debug impl reverted)
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4830 tests, 0 failures, 3/3 stable)
- Total: 5728 tests, 0 failures

## Stage Summary
- Process-per-test isolation infrastructure implemented (subprocess compile_src)
- `--check-errors` flag added to CLI (outputs JSON error counts)
- `CompileResult::empty_result()` constructor added
- Debug impl re-add attempted + REVERTED (0-2 residual non-determinism)
- 架构健康度: 9.85/10 (stable — subprocess infrastructure, Debug impl deferred)
- v0.650.0

## 下一步
- **Stage 120**: Make `compile_silent` also use subprocess + add `--emit-errors-json`
  for structured error serialization. This will give 100% process isolation.
- **Stage 121**: Re-add Debug impl bodies, verify 100 runs 0 SIGSEGV.
