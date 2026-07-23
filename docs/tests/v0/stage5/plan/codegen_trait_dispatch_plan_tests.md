# Test Plan: Stage 5.53 — Codegen Trait-Dispatch Emission Plan

> **Stage**: 5.53
> **Version**: v0.11.48 → v0.11.49
> **Test file**: `tests/v0/stage5/plan/codegen_trait_dispatch_plan_tests.rs`
> **Test count**: 12 new tests (1360 → 1372 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `CodegenTraitDispatchEmissionPlan` struct +
`build_trait_dispatch_emission_plan()` free function 的正确性。

**关键不变量**：plan 的字段与三个分别调用的结果**完全一致**——
`test_build_trait_dispatch_emission_plan_match_separate_calls` 显式交叉验证。

## 2. 覆盖场景

### 2.1 边界

- 空 TraitResolver → 全空/0
- 单个 vtable → 1 vtable spec + 1 dynptr spec + summary
- 多个 vtable → 多 specs + summary

### 2.2 字段正确性

- vtable_specs == build_vtable_global_specs() 结果（集合比较）
- dynptr_specs == build_dynptr_global_specs() 结果（集合比较）
- summary == build_trait_dispatch_emission_summary() 结果（直接比较）

### 2.3 **行为等价交叉验证**

- `test_build_trait_dispatch_emission_plan_match_separate_calls`: 调用 plan
  vs 分别调用三个 builder，断言字段一致

### 2.4 边界情况

- interner 未找到 Spur → "Trait"/"Type" 默认名
- 纯函数——不修改输入 TraitResolver

### 2.5 真实场景 + 结构体语义

- 模拟真实 TraitResolver：S impls Clone+Drop+Display → 3 vtable + 3 dynptr + 4 slots
- PartialEq/Eq 派生（summary 确定性比较 + spec 长度比较）
- 字段访问

## 3. 测试统计

- 新增: 12 tests
- 基线: 1360 tests
- 总计: 1372 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.46 (`build_vtable_global_specs`)
  - Stage 5.49 (`build_dynptr_global_specs`)
  - Stage 5.52 (`build_trait_dispatch_emission_summary`)
- 下游:
  - Stage 5.54 (codegen trait-dispatch emission refactor) — driver 调用 plan
  - Stage 5.55+ (dyn Trait MIR lowering) — 直接调用 plan

## 5. CI/CD 验证

```
cargo clean: clean (967.6 MiB removed)
cargo test: 1372 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
