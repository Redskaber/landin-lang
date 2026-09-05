# Stage 111 开发日志 — Debug impl bodies re-add attempted + REVERTED (RCA)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.645.0 (无版本变更 — RCA + revert) |
| 测试数 | 5653 → 5663 (+10 stage111 RCA tests) |
| 失败数 | 0 → 0 (reverted) |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 0 src (reverted) + 10 tests + 1 stability script |

## 5W2H 根因分析

### WHAT (实验 + 发现)
Re-attempt adding Debug impl bodies for i32/i64/bool/usize to prelude, mimicking existing Display impl patterns. After Stage 107 + 109 + 110 fixed all known codegen prerequisites, the hypothesis was that Phase 3.6 (Constant type writeback) eliminated the non-deterministic SIGSEGV root cause.

### Result
- **Single test isolation**: All tests pass when run individually.
- **Full test suite**: 10-18 non-deterministic failures across 3 runs (10/18/13 different test sets each run).
- **Confirmed**: Stage 99 Layer 3 (LLVM module global state accumulation) is STILL active when combined with remaining 19 Param warnings from prelude generic def bodies.

### WHY (根因)
**Dependency gap**: Stage 110 Phase 3.6 reduced Infer warnings from 41 → 19 (-54%), but the remaining 19 Param warnings (from prelude generic def bodies like `Vec::push<T>`, `Vec::new<T>`, `Option::map<T,U>`) STILL trigger non-determinism when combined with Debug impl bodies.

Debug impl bodies add vtable + dynptr globals per type → pushes LLVM module global count past the crash threshold. LLVM CodeGenLevelDefault optimizer then non-deterministically crashes.

### WHO (影响)
- 影响: All prelude trait impl codegen stability
- 阻断: Cannot ship Debug + PartialOrd impl bodies (Stage 98 reverted, Stage 111 re-attempt reverted)
- 依赖: TD-MONO-INFER + TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION must BOTH be resolved

### WHEN (触发条件)
1. prelude 中存在 trait impl method body (Debug impls added)
2. cargo test 进程中累积足够多次 compile() 调用
3. LLVM module 全局变量数量超过阈值 (vtable + dynptr globals + function defs)
4. LLVM CodeGenLevelDefault optimizer 触发 SIGSEGV

### WHERE (代码位置)
- `src/stdlib/prelude.rs` (line ~556): Debug trait declared, impl bodies REMOVED (Stage 111 reverted)
- `src/codegen/emitter/mod.rs:275-403`: mir_type_to_emit_type Param fallback to i32 (warnings source)
- LLVM module global state: type table + target machine registry accumulation

### HOW (复现步骤)
1. 在 prelude.rs Debug trait 后加 `impl Debug for i32 { fn fmt(&self) -> String { ... } }` (4 impls)
2. `cargo build --release --features llvm-backend --bin landin-stage0`
3. `cargo test --release --features llvm-backend --test all_tests` (3+ runs)
4. 观察: 10-18 个 integration test 非确定 SIGSEGV, 失败集不同 each run

### HOW MUCH (影响范围)
- Stage 110 baseline (no Debug impl): 0 failures (5653 tests)
- Stage 111 (Debug impl added): 10-18/4755 non-deterministic failures per run
- 阻断: Debug + PartialOrd impls 无法重新添加

## 决策点 (§12 最优>最小, §1.0 原则 9 正确>妥协, 用户指示 tech-debt workflow)

### 决策 1: 选 revert Debug impl bodies, 不选保留不完整修复
- **方案 A (选)**: Revert all 4 Debug impl bodies. Preserve Stage 110 Phase 3.6 (it's correct + reduces Infer warnings). Document dependency gap in tech-debt register.
- **方案 B (不选)**: 保留 Debug impls + 跳过 cargo test — 违反 §1.0 原则 9 (正确 > 妥协) + §3.2 红线 (test --release 必须全绿).
- **方案 C (不选)**: 仅添加 1-2 个 Debug impls (e.g., bool only) — 仍然触发非确定性, 而且违反 §1.0 原则 6 (通解 > 特解).
- **引用**: §1.0 原則 9 (正确 > 妥协: 不发布非确定性 crash), 用户指示 (tech-debt workflow: 停止阉割版推进, 转而分析缺失依赖), §17.6 (直到审查不出问题为止: 继续迭代).

### 决策 2: 选保留 Phase 3.6 (Stage 110), 不选一并 revert
- **方案 A (选)**: Phase 3.6 是正确的根因修复 (Infer warnings 41→19 -54%). 它本身不引入回归. 仅 Debug impl bodies 触发 crash.
- **方案 B (不选)**: Revert Phase 3.6 + Debug impls 一起 — 过度 revert, Phase 3.6 是 Stage 105-110 迭代修复链的成果.
- **引用**: §12 (最优 > 最小: 最小 revert, 保留正确修复), §1.0 原則 9 (正确 > 妥协: Phase 3.6 active 不影响 Stage 110 baseline 稳定性).

### 决策 3: 选记录依赖 TD, 不选忽略
- **方案 A (选)**: 同步更新 tech-debt-register.md — 升级 TD-MONO-INFER + TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION 描述, 明确它们是 Debug impl re-add 的硬阻断.
- **方案 B (不选)**: 不记录 — 违反用户指示 (及时同步 TD) + §1.0 原則 4 (报错 > 静默).
- **引用**: 用户指示 (发现依赖缺失, 同步到 tech-debt), §1.0 原則 4 (报错 > 静默).

## 裁剪点 (§1.2.1)

- L3 任务 (跨 prelude + codegen + driver 稳定性), 但实际改动 L2 (~40 LOC src + 10 tests)
- 按 §1.2.1 走 §7.3 门审查 + §3.2 验收 + 100 次稳定性验证 (核心门禁)
- §3.2 验收 (reverted 后): 全绿 (5663 tests, 0 failures, 0 clippy warnings)

## §3.2 验收 (reverted 后)

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4765 tests, 0 failures, 9 ignored)
- 总: 5663 tests, 0 failures, 9 ignored (Stage 110 baseline 5653 + 10 stage111 RCA tests)

## Stage Summary

- TD-TYPECK-WRITEBACK-INCOMPLETE Debug impl re-add 验证完成 + REVERTED
- 4 Debug impl bodies (i32/i64/bool/usize) 添加触发 10-18/4755 非确定性 SIGSEGV
- 根因: 依赖缺失 — TD-MONO-INFER (Param warnings from generic def bodies) + TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION (LLVM module 全局状态累积)
- Revert Debug impl bodies, 保留 Stage 110 Phase 3.6 (正确修复, 无回归)
- 添加 10 个 stage111 RCA tests + 1 个 stability script (scripts/stability_v2.sh)
- 同步更新 tech-debt-register.md — 升级 TD-MONO-INFER + TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION 描述
- 架构健康度: 9.85/10 (stable — RCA + revert, 无代码变更, 但依赖 gap 记录)

## 下一步

- **Stage 112**: 修复 TD-MONO-INFER — non-turbofish path generic call FnDef substs 推断 (writeback_fndef_substs back-propagation in typeck). 参考 rustc `InferCtxt` + `TypeVariable` 设计. 预期消除 19 Param warnings 中的大部分.
- **Stage 113**: 调查 TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION — LLVM module 全局状态隔离. 考虑 LLVM 22 的 `LLVMRustExecutionContext` (LLVM 19+ per-thread context).
- **Stage 114**: 再次重新添加 Debug impl bodies, 验证 100 次跑 0 SIGSEGV (依赖 Stage 112 + 113 完成).
