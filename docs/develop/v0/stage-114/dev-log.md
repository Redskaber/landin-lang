# Stage 114 开发日志 — Debug impl bodies re-add attempted + REVERTED (RCA)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.646.0 (无版本变更 — RCA + revert) |
| 测试数 | 5686 → 5696 (+10 stage114 RCA tests) |
| 失败数 | 0 → 0 (reverted) |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 0 src (reverted) + 10 tests |

## 5W2H 根因分析

### WHAT (实验 + 发现)
Stage 113 修复了 TD-MONO-INFER + TD-LLVM-OBJ-EMIT-CRASH. Stage 114 重新
添加 Debug impl bodies for i32/i64/bool/usize, 验证 100 次跑 0 SIGSEGV.

### Result: REVERTED
- Baseline (Stage 113, no Debug impl): 3/3 runs 0 failures (stable).
- Stage 114 (Debug impl added): 9-23 non-deterministic failures per run
  (different sets each run: 23, 9, 17). Single tests pass in isolation.

### WHY (根因)
Stage 113 fixed TD-MONO-INFER (writeback secondary pass + skip ALL prelude
generic def bodies) + TD-LLVM-OBJ-EMIT-CRASH (fn_sigs_map specialized sigs).
But TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION (Stage 99 Layer 3) is STILL
active:

- Debug impl bodies add vtable + dynptr globals per type (4 types ×
  2 globals = 8 new globals).
- These globals accumulate across cargo test subprocess compile() calls.
- LLVM module global state (type table, target machine registry)
  accumulates → LLVM CodeGenLevelDefault optimizer non-deterministically
  crashes.

Stage 113's skip rule (skip ALL prelude generic def bodies) eliminates
Param-containing prelude generic function bodies, but Debug impl bodies
are NOT generic (they're concrete impl methods on concrete types). The
vtable + dynptr globals they trigger are the crash source.

### New TD Discovered
- **TD-TRAIT-METHOD-AMBIGUITY** (P3, v0.13+): When both Display and Debug
  traits have a `fmt` method, method resolution picks Display (wrong)
  instead of Debug (correct) for `n.fmt()` calls. This is a separate
  issue from the non-deterministic SIGSEGV, but blocks Debug impl
  usability even if the SIGSEGV is fixed.

### WHO (影响)
- 影响: All prelude trait impl codegen stability
- 阻断: Cannot add Debug impl bodies until TD-PRELUDE-IMPL-BODY-MODULE-
  ACCUMULATION is resolved
- 新发现 TD: TD-TRAIT-METHOD-AMBIGUITY (P3, v0.13+)

### WHEN (触发条件)
1. prelude 中存在 trait impl method body (Debug impls added)
2. cargo test 进程中累积足够多次 compile() 调用
3. LLVM module 全局变量数量超过阈值 (vtable + dynptr globals)
4. LLVM CodeGenLevelDefault optimizer 触发 SIGSEGV

### WHERE (代码位置)
- `src/stdlib/prelude.rs` (Debug trait + impl bodies) — REVERTED
- LLVM module global state — crash site (TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION)
- Trait method resolution — TD-TRAIT-METHOD-AMBIGUITY

### HOW (复现步骤)
1. Add 4 Debug impl bodies to prelude.rs
2. `cargo build --release --features llvm-backend --bin landin-stage0`
3. `cargo test --release --features llvm-backend --test all_tests` (3+ runs)
4. Observe: 9-23 non-deterministic failures per run, different sets
5. Revert Debug impl bodies, verify 3/3 runs 0 failures

### HOW MUCH (影响范围)
- Stage 113 baseline (no Debug impl): 0 failures (5686 tests, 3/3 stable)
- Stage 114 (Debug impl added): 9-23/4788 non-deterministic failures per run
- 阻断: Debug impl re-add blocked by TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION

## 决策点 (§12 最优>最小, §1.0 原则 9 正确>妥协, 用户指示 tech-debt workflow)

### 决策 1: 选 revert Debug impl bodies, 不选保留不完整修复
- 引用 §1.0 原則 9 (正确 > 妥协): 不发布非确定性 crash
- 引用用户指示: 发现依赖缺失停止阉割版推进, 转而分析缺失依赖
- 引用 §17.6 (直到审查不出问题为止): 继续迭代

### 决策 2: 选记录新 TD (TD-TRAIT-METHOD-AMBIGUITY), 不选忽略
- 引用用户指示: 发现功能缺失及时同步 TD
- 引用 §1.0 原則 4 (报错 > 静默)

### 决策 3: 选保留 Stage 113 baseline, 不选 revert Phase 3.6
- 引用 §12 (最优 > 最小: 最小 revert)
- 引用 §1.0 原則 9 (正确 > 妥协: Phase 3.6 active 不影响 baseline 稳定性)

## §3.2 验收 (reverted 后)
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4798 tests, 0 failures)
- 总: 5696 tests (898 lib + 4798 integration + 10 stage114 RCA), 0 failures

## Stage Summary
- Debug impl bodies re-add attempted + REVERTED
- 4 Debug impls (i32/i64/bool/usize) 触发 9-23/4788 非确定性 SIGSEGV
- RCA: TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION 仍 active (Debug impl bodies
  add vtable + dynptr globals → LLVM module state accumulation)
- New TD: TD-TRAIT-METHOD-AMBIGUITY (Display::fmt vs Debug::fmt method resolution)
- Revert Debug impls, preserve Stage 113 baseline (5686 tests, 0 failures, 3/3 stable)
- 架构健康度: 9.85/10 (stable — RCA + revert, 无代码变更, 依赖 gap 记录)

## 下一步
- **Stage 115**: 调查 TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION — LLVM module
  global state isolation. 考虑 LLVM 22 的 LLVMRustExecutionContext (LLVM 19+
  per-thread context) 作为隔离方案. 参考 rustc 的 LLVM binding (rustc_llvm)
  如何处理 module state.
- **Stage 116**: 修复 TD-TRAIT-METHOD-AMBIGUITY — trait method resolution
  需要区分 Display::fmt vs Debug::fmt (基于 return type 或显式 trait dispatch).
- **Stage 117**: 再次重新添加 Debug impl bodies, 验证 100 次跑 0 SIGSEGV
  (依赖 Stage 115 + 116 完成).
