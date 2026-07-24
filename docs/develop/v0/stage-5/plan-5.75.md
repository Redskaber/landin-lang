# Stage 5.75 开发计划：find_dyn_trait_method_call_in_plan

> **阶段**: Stage 5.75
> **版本**: v0.11.70 → v0.11.71
> **状态**: ✅ Complete

## 1. 目标

添加 free function `find_dyn_trait_method_call_in_plan()`——在
`DynTraitMIRPlan` (Stage 5.73) 中按 `(trait_name, type_name, method_name)`
查找匹配的 `DynTraitMethodCall`。这是 `mir/lower/` 集成的**第一个查询
API**——当 HIR 中遇到 `x.method()` 且 `x` 是 `dyn Trait` 时，lowering
使用此函数从 plan 中取出对应的 method call 表示。

## 2. 背景

Stage 5.61-5.74 完成了 dyn Trait MIR 基础设施的全部数据结构与 IR 文本
生成器：
- 值表示：`DynTraitFatPtr` (5.61) + 桥接 (5.62-5.65)
- 方法调用：`DynTraitMethodCall` (5.66) + IR 文本 (5.67-5.70)
- 汇总：`DynTraitMIRSummary` (5.71-5.72)
- 计划：`DynTraitMIRPlan` (5.73)
- 完整 IR：`emit_dyn_trait_mir_plan_text` (5.74)

但所有 API 都是**整个项目级别的批量生成**。要在 `mir/lower/` 中实际
集成，需要一个**单点查询入口**——给定一个具体的 method call 描述，
从 plan 中取出对应的 `DynTraitMethodCall`。Stage 5.75 提供此入口。

## 3. 设计

### 3.1 新增 API

```rust
pub fn find_dyn_trait_method_call_in_plan<'a>(
    plan: &'a DynTraitMIRPlan,
    trait_name: &str,
    type_name: &str,
    method_name: &str,
) -> Option<&'a DynTraitMethodCall>
```

### 3.2 计算规则

1. 遍历 `plan.method_calls`
2. 对每个 `DynTraitMethodCall`，比较 `trait_name`/`type_name`/`method_name`
   三个字段（字符串相等比较）
3. 返回第一个匹配项的引用
4. 无匹配返回 `None`

### 3.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `find_dyn_trait_method_call_in_plan` | `find_<noun>_<noun>_<noun>_<prep>_<noun>` | ✅ |

参考 v1.6 (Stage 5.36) 的 `find_stdlib_trait_method` — 同为 `find_` 前缀
的查询函数，遵循 §8.1 helper-verb 约定。

### 3.4 §16 接口隔离

- 输入：`&DynTraitMIRPlan` + 3 个 `&str`
- 输出：`Option<&DynTraitMethodCall>`
- 纯只读，无副作用，无循环依赖
- 数据流：stdlib → mir/dyn_trait（已在 5.62 建立），本 stage 仅在
  mir/dyn_trait 内部新增查询入口，不引入新依赖

### 3.5 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | 空 plan，任意查询 | `None` |
| 2 | 单 method call plan，精确匹配 | `Some(...)` |
| 3 | 单 method call plan，trait_name 不匹配 | `None` |
| 4 | 单 method call plan，type_name 不匹配 | `None` |
| 5 | 单 method call plan，method_name 不匹配 | `None` |
| 6 | 多 method call plan，匹配第二项 | `Some(...)` (指向第二项) |
| 7 | 多 method call plan，匹配最后一项 | `Some(...)` (指向最后一项) |
| 8 | 多 method call plan，全部不匹配 | `None` |
| 9 | 大小写敏感：`Display` ≠ `display` | `None` |
| 10 | 同 trait+type 多方法，method_name 区分 | 正确选项 |

## 4. 集成路径（后续 Stage 预告）

Stage 5.76+ 将在 `mir/lower/mod.rs` 的 `HirExprKind::MethodCall` 分支中
调用本函数：

```rust
// 未来 mir/lower/mod.rs MethodCall 分支伪代码
if let Some(plan) = &cx.dyn_trait_plan {
    if let Some(call) = find_dyn_trait_method_call_in_plan(
        plan, trait_name, type_name, method_name) {
        // 走 dyn Trait vtable 间接调用路径
        return lower_dyn_trait_method_call(cx, call, args);
    }
}
// fallback: 现有 placeholder 路径
```

本 stage 仅提供查询 API，不修改 `mir/lower/`。

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
