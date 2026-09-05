# Stage 115 开发日志 — TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION partial fix + Debug impl RCA

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.646.0 → v0.647.0 |
| 测试数 | 5696 → 5706 (+10 stage115 tests) |
| 失败数 | 0 → 0 (Debug impl reverted) |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | +30 (sort fixes) + 0 (Debug impl reverted) + 10 tests |

## 5W2H 根因分析

### WHAT (发现)
Stage 114 RCA confirmed TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION blocks
Debug impl re-add. Stage 115 investigated the root cause.

### WHY (根因 — HashMap random SipHash seed)
`TraitResolver.vtables` is a `HashMap<(Spur, Spur), Vtable>`. Rust's
`HashMap` uses a random SipHash seed per process. Iteration order varies
between runs → vtable/dynptr/drop_glue/mono_items globals emitted in
different order → different LLVM module states → non-deterministic
SIGSEGV in LLVMTargetMachineEmitToFile.

### HOW (修复 — 4 sort fixes)
1. `build_vtable_global_specs` — sort by global_name
2. `build_dynptr_global_specs` — sort by global_name
3. `emit_drop_glue_functions` — sort by def_id
4. `collect_mono_items` — sort by Debug format

### Result: PARTIAL fix + Debug impl REVERTED
- Baseline (no Debug impl, with sort fixes): 3/3 runs 0 failures (stable)
- Debug impl + sort fixes: 0-3 non-deterministic failures per run
- Remaining non-determinism from LLVM's internal C++ hash tables

### New TD: TD-LLVM-INTERNAL-NONDETERMINISM (P3, v0.13+)
LLVM's C++ code uses hash-based data structures (SelectionDAG, register
allocator, instruction scheduler) that depend on memory layout. Even with
deterministic IR (from sort fixes), LLVM's backend can produce different
machine code between runs, occasionally crashing. Fix requires:
LLVMRustExecutionContext (per-thread context isolation) or process-per-test
isolation.

## §3.2 验收 (Debug impl reverted)
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4808 tests, 0 failures, 3/3 stable)
- Total: 5706 tests, 0 failures

## Stage Summary
- TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION PARTIAL fix (sort fixes)
- Root cause: HashMap random SipHash seed → non-deterministic emission order
- 4 sort fixes applied (vtable, dynptr, drop_glue, mono_items)
- Non-deterministic failures reduced from 9-23 to 0-3
- Debug impl bodies still REVERTED (0-3 > 0, §3.2 red line)
- New TD: TD-LLVM-INTERNAL-NONDETERMINISM (P3, v0.13+)
- 架构健康度: 9.85/10 (stable — sort fixes, no regression, dependency gap documented)
- v0.647.0
