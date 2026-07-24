# Stage 5.88 开发计划：stdlib_arithmetic_traits — 算术运算符 trait 语义分组查询

> **阶段**: Stage 5.88
> **版本**: v0.11.83 → v0.11.84
> **状态**: ✅ Complete

## 1. 目标

添加 free function `stdlib_arithmetic_traits() -> Vec<&'static str>` —— 返回
所有 stdlib 算术运算符 trait 名称列表（Add/Sub/Mul/Div/Rem/BitAnd/BitOr/BitXor/
Shl/Shr + 对应的 Assign 变体）。

这是**语义分组查询**系列的第一步——之前有 marker traits（5.87）和
vtable traits（5.37），现在添加 arithmetic traits 语义分组。

## 2. 设计动机

当前 stdlib 有：
- `stdlib_marker_traits()` (5.87) —— marker trait 批量查询
- `stdlib_traits_with_vtable()` (5.37) —— 有方法的 trait 批量查询
- `stdlib_all_traits()` (5.86) —— 所有 trait

**缺失**：按**语义类别**分组的查询。算术运算符 trait（Add/Sub/Mul/...）
是一个清晰的语义类别，常用于：
- 运算符重载检测
- 类型推断辅助（"这个类型是否支持算术运算？"）
- codegen 决定是否生成运算符调用代码

`stdlib_arithmetic_traits()` 提供这个语义分组的批量查询。

## 3. 设计

### 3.1 新增 API

```rust
/// Stage 5.88: Return all stdlib arithmetic operator trait names.
pub fn stdlib_arithmetic_traits() -> Vec<&'static str>
```

### 3.2 计算规则

返回固定列表（20 个 trait）：
- 二元算术：Add, Sub, Mul, Div, Rem, BitAnd, BitOr, BitXor, Shl, Shr
- 复合赋值：AddAssign, SubAssign, MulAssign, DivAssign, RemAssign,
  BitAndAssign, BitOrAssign, BitXorAssign, ShlAssign, ShrAssign

### 3.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `stdlib_arithmetic_traits` | `<noun>_<adj>_<noun>` (plural) | ✅ |

参考 `stdlib_marker_traits` (5.87) / `stdlib_traits_with_vtable` (5.37)
同家族——`stdlib_<category>_<noun>` 模式。

### 3.4 §16 接口隔离

- 输入：无
- 输出：`Vec<&'static str>`
- 纯只读，无新依赖
- 使用 `&'static` 切片，与现有 `STDLIB_TRAITS` 设计一致

### 3.5 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | stdlib_arithmetic_traits 非空 | ✓ |
| 2 | 包含 "Add" | ✓ |
| 3 | 包含 "Sub" | ✓ |
| 4 | 包含 "Mul" | ✓ |
| 5 | 包含 "Div" | ✓ |
| 6 | 包含 "Rem" | ✓ |
| 7 | 包含 "BitAnd" | ✓ |
| 8 | 包含 "Shl" | ✓ |
| 9 | 包含 "Shr" | ✓ |
| 10 | 包含 "AddAssign" | ✓ |
| 11 | 包含 "ShrAssign" | ✓ |
| 12 | 不包含 "Copy" (marker) | ✓ |
| 13 | 不包含 "Clone" (core trait) | ✓ |
| 14 | 不包含 "Foo" (user-defined) | ✓ |
| 15 | count == 20 (10 binary + 10 assign) | ✓ |
| 16 | ⊆ stdlib_all_traits | ✓ |
| 17 | ∩ stdlib_marker_traits == ∅ | ✓ |
| 18 | 无副作用 | ✓ |
| 19 | 无重复 | ✓ |

## 4. 不在本 stage 范围

- ❌ 其他语义分组（core/io/iterator 等留待后续 stage）
- ❌ 修改现有查询函数
- ❌ mir/lower 拆分（TD-011, Stage 6）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
