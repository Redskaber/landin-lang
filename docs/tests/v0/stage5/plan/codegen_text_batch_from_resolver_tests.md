# Test Plan: Stage 5.56 — Codegen Trait-Dispatch Emission Text Batch from Resolver

> **Stage**: 5.56
> **Version**: v0.11.51 → v0.11.52
> **Test file**: `tests/v0/stage5/plan/codegen_text_batch_from_resolver_tests.rs`
> **Test count**: 12 new tests (1396 → 1408 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `emit_trait_dispatch_globals_text_batch_from_resolver()` 便捷入口的正确性。

**关键不变量**：
1. 输出与 `emit_vtables()` + `emit_dyn_trait_ptrs()` 分别调用（通过 Emitter）的输出**逐字节一致**
2. 输出与 `emit_trait_dispatch_globals_text_batch()` (Stage 5.55) 当给定相同 resolver 的 plan 时**输出一致**

## 2. 覆盖场景

### 2.1 边界

- 空 TraitResolver → 空 Vec
- 单个 vtable → 2 行
- 多个 vtable → 多行

### 2.2 **两个行为等价交叉验证**

- `test_match_separate_emit_vtables_and_dyn_trait_ptrs`: 便捷入口 vs 分别调用 emit_vtables + emit_dyn_trait_ptrs（通过 Emitter），断言每行 text 出现在 emitter 输出中
- `test_match_plan_based_text_batch`: 便捷入口 vs plan-based text batch (Stage 5.55)，断言集合相等

### 2.3 边界情况

- 纯函数——不修改输入 resolver
- 无需 Emitter——不构造任何 Emitter trait 对象

### 2.4 顺序正确性

- vtable 行在前
- dynptr 行在后

### 2.5 计数 + 真实场景 + 确定性

- 行数 == 2 × vtables.len()
- 模拟真实场景：S impls Clone+Drop+Display → 6 行
- 重复调用相同结果

## 3. 测试统计

- 新增: 12 tests
- 基线: 1396 tests
- 总计: 1408 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.53 (`build_trait_dispatch_emission_plan`)
  - Stage 5.55 (`emit_trait_dispatch_globals_text_batch`)
  - 现有 `emit_vtables()` + `emit_dyn_trait_ptrs()` — 用于交叉验证
- 下游:
  - Stage 5.57 (codegen trait-dispatch emission refactor) — driver 调用便捷入口
  - Stage 5.58+ (dyn Trait MIR lowering) — 直接调用

## 5. CI/CD 验证

```
cargo clean: clean (1.0 GiB removed)
cargo test: 1408 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅ (修复了 1 个 unused import)
```

---

**创建日期**: 2026-07-23
