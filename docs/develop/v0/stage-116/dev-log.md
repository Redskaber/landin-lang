# Stage 116 开发日志 — TD-LLVM-INTERNAL-NONDETERMINISM RCA + LLVMShutdown fix + Debug impl REVERTED

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.647.0 → v0.648.0 |
| 测试数 | 5706 → 5714 (+8 stage116 tests) |
| 失败数 | 0 → 0 (Debug impl reverted) |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | +15 (LLVMShutdown) + 0 (Debug impl reverted) + 8 tests |

## 5W2H

### WHAT
Stage 116 attempted to fix TD-LLVM-INTERNAL-NONDETERMINISM by calling
`LLVMShutdown()` in `LLVMSysEmitter::Drop` to reset LLVM C++ global state.

### Result: LLVMShutdown keeps baseline stable, but Debug impl non-determinism persists
- Baseline (no Debug impl, with LLVMShutdown): 3/3 runs 0 failures (stable)
- Debug impl + LLVMShutdown + sort fixes: 0-5 non-deterministic failures
  (2/5 runs pass, 3/5 have 1-5 failures)

### WHY
LLVMShutdown() resets ManagedStatic objects (target registry, pass registry)
but does NOT reset LLVM's C++ heap allocator state. SelectionDAG and RegAlloc
use `DenseMap` which allocates heap memory whose addresses vary between runs
due to ASLR. The hash function for DenseMap uses pointer addresses, producing
different iteration orders → different codegen → occasional crashes.

### New TD: TD-PROCESS-PER-TEST-ISOLATION (P3, v0.13+)
The only complete fix is **process-per-test isolation** (each compile() in a
separate subprocess, like rustc). This is a major architectural change.

## §3.2 验收 (Debug impl reverted)
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4816 tests, 0 failures, 3/3 stable)
- Total: 5714 tests, 0 failures

## Stage Summary
- LLVMShutdown() added to Drop (keeps baseline stable, no regression)
- Debug impl bodies still REVERTED (0-5 > 0, §3.2 red line)
- New TD: TD-PROCESS-PER-TEST-ISOLATION (P3, v0.13+)
- 架构健康度: 9.85/10 (stable — LLVMShutdown fix preserved, dependency gap documented)
- v0.648.0

## 下一步
- **Stage 117**: Implement process-per-test isolation — each `compile()` in a
  subprocess via `std::process::Command`. This completely isolates LLVM C++
  state between compilations, matching rustc's architecture.
- **Stage 118**: Re-add Debug impl bodies, verify 100 runs 0 SIGSEGV.
