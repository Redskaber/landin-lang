# Stage 5.77 开发计划：find_dyn_trait_method_call_in_plan_by_method

> **阶段**: Stage 5.77
> **版本**: v0.11.72 → v0.11.73
> **状态**: ✅ Complete

## 1. 目标

添加 free function `find_dyn_trait_method_call_in_plan_by_method()`——
在 `DynTraitMIRPlan` 中**仅按 method_name** 查找匹配的 `DynTraitMethodCall`。
这是 Stage 5.75 (`find_dyn_trait_method_call_in_plan`) 的**模糊查询伙伴**——
当 `mir/lower/` 在 `HirExprKind::MethodCall` 分支处理表达式时，
HIR 层尚未解析 receiver 的具体 trait/type（那是 typeck 的职责），
只能从 HIR 拿到 method_name。

Stage 5.77 提供此模糊查询入口；Stage 5.78 才会在 `lower_expr_to_operand`
的 MethodCall 分支实际调用它。

## 2. 设计动机

Stage 5.75 的 `find_dyn_trait_method_call_in_plan(plan, trait, type, method)`
要求调用方提供**完整的 (trait, type, method) 三元组**。在 MIR lowering
时这不可行——`MirLowerCtxt` 不持有 TraitResolver，HIR 层也不暴露
receiver 的具体 dyn Trait 类型（那是 typeck 推断后才知道的）。

但 lower 阶段确实能拿到 `method.name: Symbol`（从 HIR `MethodCall.method`）。
所以需要一个**仅按 method_name** 查询的变体。

**适用场景**：当项目里每个 method_name 唯一时（例如：`drop` 只在 `Drop` trait 上、
`clone` 只在 `Clone` trait 上），模糊查询足够定位。当有歧义时（多个 trait
都有同名方法），返回第一个匹配项——这是设计权衡，由调用方决定是否接受。

## 3. 设计

### 3.1 新增 API

```rust
pub fn find_dyn_trait_method_call_in_plan_by_method<'a>(
    plan: &'a DynTraitMIRPlan,
    method_name: &str,
) -> Option<&'a DynTraitMethodCall>
```

### 3.2 计算规则

1. 遍历 `plan.method_calls`
2. 比较 `method_name` 字段（字符串相等比较，大小写敏感）
3. 返回第一个匹配项的引用
4. 无匹配返回 `None`

### 3.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `find_dyn_trait_method_call_in_plan_by_method` | `find_<noun>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` | ✅ |

参考 Stage 5.75 的 `find_dyn_trait_method_call_in_plan` —— 同 `find_` 前缀家族，
`_by_method` 后缀明确表达"按 method 字段过滤"的语义。这是 Rust API guidelines
推荐的命名模式（如 `slice::iter_by`、`HashMap::get_by`）。

### 3.4 §16 接口隔离

- 输入：`&DynTraitMIRPlan` + `&str`
- 输出：`Option<&DynTraitMethodCall>`
- 纯只读，无副作用，无循环依赖
- 与 5.75 同住 `mir::dyn_trait`，不引入新依赖

### 3.5 与 5.75 的关系

| 5.75 (精确) | 5.77 (模糊) |
|------------|------------|
| 输入：(trait, type, method) 三元组 | 输入：method 单字段 |
| 适用：调用方已知道 trait/type | 适用：调用方只知道 method_name |
| 唯一性：精确匹配，无歧义 | 唯一性：first-match-wins，可能有歧义 |
| 使用场景：driver/typeck 之后 | 使用场景：MIR lower 之前/期间 |

两者**互补**，不互相替代。Stage 5.78+ 在 `mir/lower/` 使用 5.77 的模糊查询，
因为 HIR 层不暴露 receiver 的具体 trait/type。

### 3.6 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | 空 plan，任意 method | `None` |
| 2 | 单 method call plan，method 匹配 | `Some(...)` |
| 3 | 单 method call plan，method 不匹配 | `None` |
| 4 | 多 method call plan，匹配第一项 | `Some(...)` (指向第一项) |
| 5 | 多 method call plan，匹配中间项 | `Some(...)` (指向中间项) |
| 6 | 多 method call plan，匹配最后一项 | `Some(...)` (指向最后一项) |
| 7 | 多 method call plan，全部不匹配 | `None` |
| 8 | 大小写敏感：`drop` ≠ `Drop` | `None` |
| 9 | 同名方法跨多个 trait，first-match-wins | 返回第一项 |
| 10 | 同名方法跨多个 type，first-match-wins | 返回第一项 |
| 11 | 与 5.75 精确查询一致性：相同 method + 唯一 trait/type 时结果一致 | 一致 |

## 4. 不在本 stage 范围

- ❌ 不修改 `HirExprKind::MethodCall` 分支（Stage 5.78）
- ❌ 不在 `mir/lower/` 中调用本函数（Stage 5.78）
- ❌ 不解决 method_name 歧义（设计上接受 first-match-wins）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
