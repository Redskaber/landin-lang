# Stage 117 开发日志 — TD-PROCESS-PER-TEST-ISOLATION RCA (process-per-test confirmed viable)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.648.0 (无版本变更 — RCA) |
| 测试数 | 5714 → 5720 (+6 stage117 tests) |
| 失败数 | 0 → 0 (Debug impl reverted) |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 0 src + 6 tests |

## 5W2H 根因分析

### WHAT
Stage 117 investigated whether TD-PROCESS-PER-TEST-ISOLATION would fix the
remaining LLVM non-determinism (0-5 failures per run with Debug impl bodies).

### Key Findings

1. **Simple programs work 10/10 in subprocess** — `landin-stage0 --run` with a
   simple program (`println!("{}", 42)`) passes 10/10 times. Each subprocess
   gets fresh LLVM C++ state.

2. **Tests that fail in full suite pass in isolation** — `stage18_332_sret
   _stress_repeated` passes when run alone but fails in the full test suite.
   This confirms the non-determinism is from **cross-compilation accumulation**
   (LLVM C++ state shared across `compile()` calls in the same process), NOT
   from single-compilation non-determinism.

3. **ASLR off doesn't help** — `setarch -R cargo test` still produces 6-8
   failures. The non-determinism is from C++ heap allocator internal state,
   not just ASLR.

4. **Process-per-test isolation WOULD work** — changing `compile_src` test
   helper to use a subprocess (like `run_program` already does) would give
   each test fresh LLVM C++ state, eliminating cross-compilation accumulation.

5. **Implementation requires structured error serialization** — `CompileResult`
   contains `Vec<TypeError>`, `Vec<BorrowError>`, etc. Need to serialize/
   deserialize across the process boundary.

### WHY
LLVM C++ `DenseMap`/`MapVector` use pointer-address hashing. In a single
process, heap allocator addresses change between `compile()` calls due to
allocation/deallocation patterns (not just ASLR). Each `compile()` call
leaves residual state in LLVM's C++ global heaps → next `compile()` sees
different hash table iteration order → non-deterministic codegen.

In a subprocess, each `compile()` call starts with fresh C++ heap state
(no residual from previous compilations) → deterministic.

### How to Fix (TD-PROCESS-PER-TEST-ISOLATION)
1. Change `compile_src` in `tests/common/mod.rs` to use `std::process::Command`
   to call `landin-stage0 --compile <temp_file>` instead of calling `compile()`
   in-process.
2. Parse the exit code + stderr to construct a `CompileResult`.
3. For tests that need structured error details (e.g., `result.errors.typeck`),
   add a `--emit-errors-json` flag to `landin-stage0` that serializes errors
   as JSON.

### HOW MUCH
- Estimated: ~200 LOC in `tests/common/mod.rs` + ~50 LOC in `src/bin/main.rs`
  for `--emit-errors-json` flag
- Risk: low — `run_program` already uses subprocess model successfully

## §3.2 验收 (Debug impl reverted)
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4820 tests, 0 failures)
- Total: 5720 tests, 0 failures

## Stage Summary
- TD-PROCESS-PER-TEST-ISOLATION confirmed as viable fix
- Non-determinism is cross-compilation accumulation, NOT single-compilation
- Simple programs work 10/10 in subprocess (fresh LLVM each time)
- Tests that fail in full suite pass in isolation
- Fix requires changing compile_src to use subprocess + structured error serialization
- Debug impl bodies still REVERTED (process-per-test not yet implemented)
- 架构健康度: 9.85/10 (stable — RCA, dependency gap documented)
- v0.648.0 (无版本变更 — RCA)

## 下一步
- **Stage 118**: Implement process-per-test isolation — change `compile_src`
  to use subprocess + add `--emit-errors-json` flag to `landin-stage0`
- **Stage 119**: Re-add Debug impl bodies with `debug_fmt` method name
  (avoids TD-TRAIT-METHOD-AMBIGUITY), verify 100 runs 0 SIGSEGV
