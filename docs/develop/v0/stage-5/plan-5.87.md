# Stage 5.87 开发计划：stdlib_marker_traits — marker trait 列表查询

> **阶段**: Stage 5.87
> **版本**: v0.11.82 → v0.11.83
> **状态**: ✅ Complete

## 1. 目标

添加 free function `stdlib_marker_traits() -> Vec<&'static str>` —— 返回
所有 stdlib marker trait 名称列表（Copy/Send/Sync/Sized/Unpin/Eq）。

与 `stdlib_traits_with_vtable()`（返回有方法的 trait）对称，提供
**marker trait 的批量查询**。

## 2. 设计动机

当前 stdlib 有：
- `is_stdlib_marker_trait(name) -> bool` —— 单个 marker 检查
- `stdlib_traits_with_vtable() -> Vec<&str>` —— 有方法的 trait 列表
- `stdlib_all_traits() -> Vec<&str>` —— 所有 trait（Stage 5.86）

**缺失**：一个返回**所有 marker trait** 的批量查询。当前需要遍历
`stdlib_all_traits()` + filter `is_stdlib_marker_trait` 才能得到，不直观。

`stdlib_marker_traits()` 填补这个空缺，让调用方一次获取完整 marker 列表。

## 3. 设计

### 3.1 新增 API

```rust
/// Stage 5.87: Return all stdlib marker trait names.
pub fn stdlib_marker_traits() -> Vec<&'static str>
```

### 3.2 计算规则

遍历 `STDLIB_TRAITS`（Stage 5.86 提取的模块级常量），filter 出
`is_stdlib_marker_trait` 返回 true 的 trait。

### 3.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `stdlib_marker_traits` | `<noun>_<noun>_<noun>` (plural) | ✅ |

参考 v1.6 (Stage 5.36) 的 `stdlib_traits_with_method` / `stdlib_traits_with_vtable`
同家族——`stdlib_<filter>_<noun>` 模式。

### 3.4 §16 接口隔离

- 输入：无
- 输出：`Vec<&'static str>`
- 纯只读，复用 `STDLIB_TRAITS` + `is_stdlib_marker_trait`，无新依赖

### 3.5 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | stdlib_marker_traits 非空 | ✓ |
| 2 | stdlib_marker_traits 包含 "Copy" | ✓ |
| 3 | stdlib_marker_traits 包含 "Send" | ✓ |
| 4 | stdlib_marker_traits 包含 "Sync" | ✓ |
| 5 | stdlib_marker_traits 包含 "Sized" | ✓ |
| 6 | stdlib_marker_traits 包含 "Unpin" | ✓ |
| 7 | stdlib_marker_traits 包含 "Eq" | ✓ |
| 8 | stdlib_marker_traits 不包含 "Clone" (method trait) | ✓ |
| 9 | stdlib_marker_traits 不包含 "Foo" (user-defined) | ✓ |
| 10 | stdlib_marker_traits.len() == 6 (6 markers) | ✓ |
| 11 | 与 is_stdlib_marker_trait 一致性 | 每个 trait → true |
| 12 | stdlib_marker_traits ⊆ stdlib_all_traits | ✓ |
| 13 | stdlib_marker_traits ∩ stdlib_traits_with_vtable == ∅ | ✓ |
| 14 | 无副作用——重复调用结果一致 | ✓ |
| 15 | 无重复 | ✓ |

## 4. 不在本 stage 范围

- ❌ 修改现有查询函数
- ❌ 用户自定义 marker trait 支持
- ❌ mir/lower 拆分（TD-011, Stage 6）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
