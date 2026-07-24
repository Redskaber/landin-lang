# Stage 5.73 开发计划：DynTraitMIRPlan

> **阶段**: Stage 5.73
> **版本**: v0.11.68 → v0.11.69
> **状态**: ✅ Complete

## 1. 目标

添加 `DynTraitMIRPlan` 结构体 + `build_dyn_trait_mir_plan()` 函数——
**最终聚合 API**，一次调用返回 dyn Trait MIR 所需的全部信息：fat_ptrs +
method_calls + summary。与 codegen 层的
`CodegenTraitDispatchEmissionPlan` (Stage 5.53) 对称。

## 2. 设计

### 2.1 新增类型

```rust
pub struct DynTraitMIRPlan {
    pub fat_ptrs: Vec<DynTraitFatPtr>,
    pub method_calls: Vec<DynTraitMethodCall>,
    pub summary: DynTraitMIRSummary,
}
```

### 2.2 新增 API

```rust
pub fn build_dyn_trait_mir_plan(fat_ptrs: &[DynTraitFatPtr], method_calls: &[DynTraitMethodCall]) -> DynTraitMIRPlan
pub fn build_dyn_trait_mir_plan_from_resolver(trait_resolver: &TraitResolver, interner: &Rodeo) -> DynTraitMIRPlan
```

### 2.3 命名标准化

| 类型/API | 命名规则 | 合规 |
|----------|---------|------|
| `DynTraitMIRPlan` | `<Noun><Noun><Noun><Noun>` | ✅ |
| `build_dyn_trait_mir_plan` | `<verb>_<noun>_<noun>_<noun>` | ✅ |
| `build_dyn_trait_mir_plan_from_resolver` | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` | ✅ |

### 2.4 §16 接口隔离

输入 `&[DynTraitFatPtr]` + `&[DynTraitMethodCall]` 或 `&TraitResolver` + `&Rodeo`，
输出 `DynTraitMIRPlan`。无循环依赖。

---

**创建日期**: 2026-07-24
