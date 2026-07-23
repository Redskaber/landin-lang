# Test Plan: Stage 5.49 — Codegen Dynptr Spec Builder

> **Stage**: 5.49
> **Version**: v0.11.44 → v0.11.45
> **Test file**: `tests/v0/stage5/plan/codegen_dynptr_spec_builder_tests.rs`
> **Test count**: 12 new tests (1310 → 1322 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `StdlibDynptrGlobalSpec` struct + `build_dynptr_global_specs()` free function
的正确性。

**关键不变量**：输出与 `emit_dyn_trait_ptrs()` 当前内联构造逻辑**逐字节一致**——
`test_build_dynptr_global_specs_match_emit_dyn_trait_ptrs` 显式交叉验证
（手动内联相同逻辑并断言集合相等）。

## 2. 覆盖场景

### 2.1 边界

- 空 TraitResolver → 空 Vec
- 单个 vtable → 1 spec
- 多个 vtable → multi specs（HashMap 顺序非确定，用集合比较）

### 2.2 格式组件

- global_name 格式 `.dynptr.<trait>.<type>`
- data_symbol 格式 `.data.<type>`
- vtable_symbol 格式 `.vtable.<trait>.<type>`

### 2.3 边界情况

- interner 未找到 Spur → "Trait"/"Type" 默认值（用 fresh Rodeo 模拟）
- 纯函数——不修改输入 TraitResolver
- 确定性——重复调用返回相同结果（spec 内容，HashMap 顺序可能不同）

### 2.4 一致性

- **与 emit_dyn_trait_ptrs 内联构造一致**：手动内联 `emit_dyn_trait_ptrs()`
  的构造逻辑，断言 build_dynptr_global_specs 输出的 spec 集合与之相等

### 2.5 集成

- build + emit_dynptr_global_text → 完整 LLVM IR 文本（模拟 Stage 5.50 重构后的流程）

### 2.6 真实场景

- 模拟真实 TraitResolver：struct S impls Clone + Drop + Display → 3 specs（共享 .data.S）

## 3. 测试统计

- 新增: 12 tests
- 基线: 1310 tests
- 总计: 1322 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.48 (`emit_dynptr_global_text`)
  - Stage 5.46 (`build_vtable_global_specs` — 设计对称参考)
  - 现有 `emit_dyn_trait_ptrs()` (Stage 5.7) — 用于交叉验证
- 下游:
  - Stage 5.50 (codegen dynptr emission refactor) — `emit_dyn_trait_ptrs()`
    调用 `build_dynptr_global_specs()` + 批量 push
  - Stage 5.51+ (dyn Trait MIR lowering) — 直接调用

## 5. CI/CD 验证

```
cargo clean: clean (828.0 MiB removed)
cargo test: 1322 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
