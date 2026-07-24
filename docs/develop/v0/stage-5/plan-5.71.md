# Stage 5.71 开发计划：DynTraitMIRSummary

> **阶段**: Stage 5.71
> **版本**: v0.11.66 → v0.11.67
> **状态**: ✅ Complete

## 1. 目标

添加 `DynTraitMIRSummary` 结构体 + `build_dyn_trait_mir_summary()` 函数——
项目级 dyn Trait MIR 数据汇总：fat ptr 数 + method call 数 + 涉及的 trait/type
名 + slot 总数。这是 dyn Trait MIR 基础设施的**汇总报告**，为 driver 诊断
输出和后续 MIR lowering 集成做准备。

## 2. 设计

### 2.1 新增类型

```rust
pub struct DynTraitMIRSummary {
    pub fat_ptr_count: u32,
    pub method_call_count: u32,
    pub total_slots: u32,
    pub trait_names: Vec<String>,
    pub type_names: Vec<String>,
}
```

### 2.2 新增 API

```rust
pub fn build_dyn_trait_mir_summary(
    fat_ptrs: &[DynTraitFatPtr],
    method_calls: &[DynTraitMethodCall],
) -> DynTraitMIRSummary
```

### 2.3 命名标准化

| 类型/API | 命名规则 | 合规 |
|----------|---------|------|
| `DynTraitMIRSummary` | `<Noun><Noun><Noun><Noun>` | ✅ |
| `build_dyn_trait_mir_summary` | `<verb>_<noun>_<noun>_<noun>` | ✅ |

### 2.4 §16 接口隔离

输入 `&[DynTraitFatPtr]` + `&[DynTraitMethodCall]`，输出 `DynTraitMIRSummary`。
无循环依赖。

---

**创建日期**: 2026-07-24
