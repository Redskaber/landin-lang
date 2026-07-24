# Stage 5.95 开发计划：stdlib_trait_methods_by_self_kind — 按 self_kind 反向查询

> **阶段**: Stage 5.95
> **版本**: v0.11.90 → v0.11.91
> **状态**: ✅ Complete

## 1. 目标

添加 free function `stdlib_trait_methods_by_self_kind(kind) -> Vec<(&'static str, &'static str)>` ——
返回所有具有指定 self_kind 的 stdlib trait 方法（trait_name, method_name 对）。

这是**反向查询**——给定 self_kind，找出所有匹配的方法。与
`stdlib_trait_method_self_kind` (5.94, 正向查询单个方法的 self_kind) 互补。

## 2. 设计动机

当前有：
- `stdlib_trait_method_self_kind(trait, method) -> Option<StdlibSelfKind>` (5.94) —— 正向查询
- `stdlib_traits_with_method(method_name) -> Vec<&str>` (5.36) —— 按方法名反向查询

**缺失**：按 self_kind 反向查询。这在以下场景有用：
- Codegen：找出所有 `self by value` 的方法（需要 copy receiver）
- Typeck：验证 self kind 一致性
- 文档生成：按接收者类型列出方法

## 3. 设计

### 3.1 新增 API

```rust
/// Stage 5.95: Find all stdlib trait methods with a given self_kind.
pub fn stdlib_trait_methods_by_self_kind(
    kind: StdlibSelfKind,
) -> Vec<(&'static str, &'static str)>
```

### 3.2 计算规则

遍历 `STDLIB_TRAITS`，对每个 trait 获取 `stdlib_trait_methods()`，filter 出
`method.self_kind == kind` 的方法，收集 `(trait_name, method_name)` 对。

### 3.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `stdlib_trait_methods_by_self_kind` | `<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` (plural) | ✅ |

参考 `stdlib_traits_with_method` (5.36) / `stdlib_traits_with_vtable` (5.37)
同家族——`stdlib_<noun>_<prep>_<filter>` 模式。`_by_self_kind` 后缀遵循
Rust API guidelines 的字段过滤命名约定（与 5.77 的 `_by_method` 一致）。

### 3.4 §16 接口隔离

- 输入：`StdlibSelfKind`
- 输出：`Vec<(&'static str, &'static str)>`
- 纯只读，复用 `STDLIB_TRAITS` + `stdlib_trait_methods`，无新依赖

### 3.5 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | SelfByRef 返回非空（Clone/Display/PartialEq 等都是 by ref） | ✓ |
| 2 | SelfByMutRef 返回非空（Drop/clone_from 等） | ✓ |
| 3 | SelfByValue 返回非空（算术运算符 Add/Sub 等） | ✓ |
| 4 | NoSelf 返回非空（Default::default） | ✓ |
| 5 | SelfByRef 包含 ("Clone", "clone") | ✓ |
| 6 | SelfByMutRef 包含 ("Drop", "drop") | ✓ |
| 7 | NoSelf 包含 ("Default", "default") | ✓ |
| 8 | 所有返回的方法 self_kind 匹配查询参数 | ✓ |
| 9 | 无副作用 | ✓ |
| 10 | SelfByRef 数量 > SelfByMutRef 数量（更多方法用 by ref） | ✓ |

## 4. 不在本 stage 范围

- ❌ 其他字段的反向查询（return_kind/param_count 等）
- ❌ 用户自定义 trait 支持（TD-018）
- ❌ mir/lower 拆分（TD-011）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
