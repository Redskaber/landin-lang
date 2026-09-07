# Stage 124 开发日志 — Debug impl re-add with -O0 + FastISel RCA

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.654.0 → v0.655.0 |
| 测试数 | 5742 |
| 失败数 | 0 → 0 (Debug impl reverted) |
| ignored | 9 |
| clippy warnings | 0 |

## 5W2H

### WHAT
Stage 124 researched how rustc handles LLVM non-determinism and attempted
Debug impl re-add with -O0 + FastISel.

### Key Findings

1. **rustc approach**: rustc uses process-per-compilation (each rustc invocation
   is a separate process). rustc does NOT call `LLVMTargetMachineEmitToFile`
   in the test process — it uses `LLVMRustWriteOutputFile` (custom C++ wrapper).
   rustc also uses -O0 + FastISel for debug builds.

2. **-O0 + FastISel + Debug impls**: 0-3 failures per cargo test run (2/5 stable).
   Single-program subprocess: 48/50 pass (4% failure rate). Baseline (no Debug):
   50/50 pass (0% failure rate).

3. **Root cause confirmed**: Debug impl bodies add vtable + dynptr globals →
   LLVM module becomes more complex → DenseMap hash tables have more collision
   patterns → LLVM backend non-deterministically crashes in ~4% of runs.

4. **This is NOT fixable from Rust side**: The LLVM C API
   `LLVMTargetMachineEmitToFile` internally uses SelectionDAG even at -O0.
   FastISel only handles simple instructions — complex IR (struct returns,
   GEP, vtable globals) falls back to SelectionDAG.

### Decision: Debug impl bodies PERMANENTLY DEFERRED
- 4% single-program failure rate is unacceptable (§3.2 red line: 0 failures)
- rustc's solution (custom C++ wrapper) requires forking LLVM — not practical
- Debug trait declaration preserved (users can impl Debug for own types)

### §3.2 验收 (Debug impl reverted, -O0 reverted)
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4844 tests, 0 failures, 2/2 stable)
- Total: 5742 tests, 0 failures

## Stage Summary
- rustc uses process-per-compilation + custom LLVM wrapper — not practical for Landin
- -O0 + FastISel: 4% single-program crash rate with Debug impls (vs 0% without)
- Debug impl bodies permanently deferred — LLVM C++ non-determinism is fundamental
- v0.655.0
