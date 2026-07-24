# Stage 5.76 开发计划：MirLowerCtxt dyn_trait_plan 字段 + setter

> **阶段**: Stage 5.76
> **版本**: v0.11.71 → v0.11.72
> **状态**: ✅ Complete

## 1. 目标

为 `MirLowerCtxt` 添加 `dyn_trait_plan: Option<DynTraitMIRPlan>` 字段 +
`set_dyn_trait_plan(&mut self, plan)` 设置器 + `dyn_trait_plan()` 只读
getter。这是 `mir/lower/` 集成 dyn Trait 的**第一步——仅上下文接线，
不修改 lowering 逻辑**。Stage 5.77 才会在 `HirExprKind::MethodCall` 分支
实际使用此字段。

## 2. 设计动机

Stage 5.75 提供了 `find_dyn_trait_method_call_in_plan()` —— 单点查询 API。
但 `MirLowerCtxt` 当前**没有任何字段持有 `DynTraitMIRPlan`**。要集成，
有两种选择：

1. **在每个 MethodCall 处用 resolver 现场构建 plan** — 性能差，且
   `MirLowerCtxt` 没有 resolver 引用。
2. **在 lower 入口预构建 plan，存入 cx 字段，MethodCall 分支查询** —
   清晰、高效、符合 §16（plan 由 driver 预构建，lower 仅读）。

Stage 5.76 选择方案 2。但**只添加字段 + setter/getter，不修改 lowering
逻辑**，保持 stage 增量可审查。

## 3. 设计

### 3.1 字段添加

```rust
pub struct MirLowerCtxt<'a> {
    // ... existing fields ...
    /// Stage 5.76: optional DynTraitMIRPlan for dyn Trait method call
    /// lowering. When set, the `HirExprKind::MethodCall` branch (Stage 5.77+)
    /// can query this plan via `find_dyn_trait_method_call_in_plan()` to
    /// retrieve the vtable slot index + param count for a dyn Trait method
    /// call.
    ///
    /// Per §16: the plan is built **upstream** (by the driver, using
    /// `build_dyn_trait_mir_plan_from_resolver()`) and passed in as a
    /// read-only reference. `MirLowerCtxt` does not own a TraitResolver.
    pub dyn_trait_plan: Option<DynTraitMIRPlan>,
}
```

### 3.2 新增方法

```rust
impl<'a> MirLowerCtxt<'a> {
    /// Stage 5.76: Attach a pre-built DynTraitMIRPlan to this lowering
    /// context. Subsequent MethodCall lowering (Stage 5.77+) will query
    /// this plan via `find_dyn_trait_method_call_in_plan()`.
    ///
    /// Per §16: plan is built upstream by the driver via
    /// `build_dyn_trait_mir_plan_from_resolver()`.
    pub fn set_dyn_trait_plan(&mut self, plan: DynTraitMIRPlan) {
        self.dyn_trait_plan = Some(plan);
    }

    /// Stage 5.76: Read-only access to the attached DynTraitMIRPlan, if any.
    pub fn dyn_trait_plan(&self) -> Option<&DynTraitMIRPlan> {
        self.dyn_trait_plan.as_ref()
    }
}
```

### 3.3 构造函数更新

`MirLowerCtxt::new()` 初始化 `dyn_trait_plan: None`。

### 3.4 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `set_dyn_trait_plan` | `<verb>_<noun>_<noun>_<noun>` (setter) | ✅ |
| `dyn_trait_plan` | `<noun>_<noun>_<noun>` (getter, 无前缀) | ✅ |

参考 §3.3 prefix rules 和 §8.1 helper verbs —— setter 用 `set_` 前缀，
getter 直接用字段名（Rust 习惯）。

### 3.5 §16 接口隔离

- `DynTraitMIRPlan` 在 `mir::dyn_trait` 中定义（Stage 5.73）
- `MirLowerCtxt` 在 `mir::lower` 中定义
- `mir::lower` 已经 `use crate::mir::dyn_trait::*` （通过 `crate::mir::*`）
- 数据流：driver 构建 plan → set 到 cx → cx 在 lower 中只读访问
- **无循环依赖**：mir::lower → mir::dyn_trait 单向

### 3.6 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | new() 默认无 plan | `cx.dyn_trait_plan()` 返回 None |
| 2 | set 后立即 get | 返回 Some(&plan) |
| 3 | set 后字段内容正确 | plan.fat_ptrs 长度匹配 |
| 4 | set 后字段内容正确 | plan.method_calls 长度匹配 |
| 5 | set 后字段内容正确 | plan.summary 字段匹配 |
| 6 | set 两次，第二次覆盖 | 返回第二次的 plan |
| 7 | 默认 lower 入口未设置 plan | `lower_hir_body_to_mir` 不调用 setter |
| 8 | set 后再 set None 不允许（无 unset 方法） | 设计明确：不可清除 |
| 9 | cx.mir 字段与 plan 字段独立 | set plan 不影响 mir |
| 10 | set 后跨 lower 调用可访问 | cx 生命周期内可重复 get |

## 4. 不在本 stage 范围

- ❌ 不修改 `HirExprKind::MethodCall` 分支（Stage 5.77+）
- ❌ 不修改 `lower_hir_body_to_mir_full` 自动设置 plan（Stage 5.78+ 由
  driver 接入）
- ❌ 不在 driver 中调用 `set_dyn_trait_plan`（Stage 5.78+）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
