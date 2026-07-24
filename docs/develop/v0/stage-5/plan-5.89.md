# Stage 5.89 开发计划：stdlib_core_traits — 核心 trait 语义分组查询

> **阶段**: Stage 5.89
> **版本**: v0.11.84 → v0.11.85
> **状态**: ✅ Complete

## 1. 目标

添加 free function `stdlib_core_traits() -> Vec<&'static str>` —— 返回
所有 stdlib 核心 trait 名称列表（Clone/Drop/Default/Display/Debug/
PartialEq/PartialOrd/Ord/Hash/Deref/DerefMut/IntoIterator/Iterator）。

这是**语义分组查询系列**的第二步——继 5.88 的 arithmetic traits 之后，
添加 core traits 语义分组。

## 2. 设计动机

Stage 5.88 添加了 `stdlib_arithmetic_traits()`（算术运算符类别）。
本 stage 添加 `stdlib_core_traits()`（核心 trait 类别），包括：
- 生命周期管理：Clone, Drop, Default
- 格式化：Display, Debug
- 比较：PartialEq, PartialOrd, Ord, Eq（marker，已在 markers）
- 哈希：Hash
- 解引用：Deref, DerefMut
- 迭代：IntoIterator, Iterator

这些是 Landin 程序最常用的 trait，按语义分组便于：
- 类型检查器快速判断"这个类型支持哪些核心操作？"
- 文档生成工具列出类型的 core trait impl
- codegen 决定是否生成特定的运行时支持代码

## 3. 设计

### 3.1 新增 API

```rust
/// Stage 5.89: Return all stdlib core trait names.
pub fn stdlib_core_traits() -> Vec<&'static str>
```

### 3.2 计算规则

返回固定列表（13 个 trait）：
- Clone, Drop, Default, Display, Debug
- PartialEq, PartialOrd, Ord, Hash
- Deref, DerefMut
- IntoIterator, Iterator

### 3.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `stdlib_core_traits` | `<noun>_<adj>_<noun>` (plural) | ✅ |

参考 `stdlib_arithmetic_traits` (5.88) / `stdlib_marker_traits` (5.87)
同家族——`stdlib_<category>_<noun>` 模式。

### 3.4 §16 接口隔离

- 输入：无
- 输出：`Vec<&'static str>`
- 纯只读，无新依赖
- 使用 `&'static` 切片，与 5.88 设计一致

### 3.5 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | stdlib_core_traits 非空 | ✓ |
| 2 | 包含 "Clone" | ✓ |
| 3 | 包含 "Drop" | ✓ |
| 4 | 包含 "Default" | ✓ |
| 5 | 包含 "Display" | ✓ |
| 6 | 包含 "Debug" | ✓ |
| 7 | 包含 "PartialEq" | ✓ |
| 8 | 包含 "Ord" | ✓ |
| 9 | 包含 "Hash" | ✓ |
| 10 | 包含 "Deref" | ✓ |
| 11 | 包含 "Iterator" | ✓ |
| 12 | 不包含 "Copy" (marker) | ✓ |
| 13 | 不包含 "Add" (arithmetic) | ✓ |
| 14 | 不包含 "Foo" (user-defined) | ✓ |
| 15 | count == 13 | ✓ |
| 16 | ⊆ stdlib_all_traits | ✓ |
| 17 | ∩ stdlib_marker_traits == ∅ | ✓ |
| 18 | ∩ stdlib_arithmetic_traits == ∅ | ✓ |
| 19 | 无副作用 | ✓ |
| 20 | 无重复 | ✓ |

## 4. 不在本 stage 范围

- ❌ 其他语义分组（io/unary 等留待后续 stage）
- ❌ 修改现有查询函数
- ❌ mir/lower 拆分（TD-011, Stage 6）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
