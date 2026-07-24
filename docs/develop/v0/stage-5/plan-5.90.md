# Stage 5.90 开发计划：stdlib_io_traits + stdlib_unary_traits — 小语义分组查询

> **阶段**: Stage 5.90
> **版本**: v0.11.85 → v0.11.86
> **状态**: ✅ Complete

## 1. 目标

添加两个小语义分组查询函数：
1. `stdlib_io_traits() -> Vec<&'static str>` —— 返回 I/O trait（Read, Write）
2. `stdlib_unary_traits() -> Vec<&'static str>` —— 返回一元运算符 trait（Neg, Not）

这两个类别都只有 2 个 trait，合并到一个 stage 完成语义分组系列的收尾。

## 2. 设计动机

语义分组系列进度：
- 5.87: stdlib_marker_traits (6 markers)
- 5.88: stdlib_arithmetic_traits (20 arithmetic)
- 5.89: stdlib_core_traits (13 core)
- **5.90: stdlib_io_traits (2 io) + stdlib_unary_traits (2 unary) ← 本 stage**

完成后，stdlib 的所有 trait 都有语义分组查询覆盖。

## 3. 设计

### 3.1 新增 API

```rust
/// Stage 5.90: Return all stdlib I/O trait names.
pub fn stdlib_io_traits() -> Vec<&'static str>

/// Stage 5.90: Return all stdlib unary operator trait names.
pub fn stdlib_unary_traits() -> Vec<&'static str>
```

### 3.2 计算规则

- `stdlib_io_traits`: 返回 ["Read", "Write"]
- `stdlib_unary_traits`: 返回 ["Neg", "Not"]

### 3.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `stdlib_io_traits` | `<noun>_<adj>_<noun>` (plural) | ✅ |
| `stdlib_unary_traits` | `<noun>_<adj>_<noun>` (plural) | ✅ |

参考 `stdlib_core_traits` (5.89) / `stdlib_arithmetic_traits` (5.88) 同家族。

### 3.4 §16 接口隔离

- 输入：无
- 输出：`Vec<&'static str>`
- 纯只读，无新依赖
- 使用 `&'static` 切片

### 3.5 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | stdlib_io_traits 非空 | ✓ |
| 2 | stdlib_io_traits 包含 "Read" | ✓ |
| 3 | stdlib_io_traits 包含 "Write" | ✓ |
| 4 | stdlib_io_traits count == 2 | ✓ |
| 5 | stdlib_io_traits ⊆ all_traits | ✓ |
| 6 | stdlib_unary_traits 非空 | ✓ |
| 7 | stdlib_unary_traits 包含 "Neg" | ✓ |
| 8 | stdlib_unary_traits 包含 "Not" | ✓ |
| 9 | stdlib_unary_traits count == 2 | ✓ |
| 10 | stdlib_unary_traits ⊆ all_traits | ✓ |
| 11 | io ∩ markers == ∅ | ✓ |
| 12 | unary ∩ arithmetic == ∅ | ✓ |
| 13 | 无副作用（两个函数） | ✓ |
| 14 | 无重复（两个函数） | ✓ |

## 4. 不在本 stage 范围

- ❌ 修改现有查询函数
- ❌ mir/lower 拆分（TD-011, Stage 6）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
