# Stage 5.86 开发计划：stdlib_trait_count + stdlib_all_traits 便利查询

> **阶段**: Stage 5.86
> **版本**: v0.11.81 → v0.11.82
> **状态**: ✅ Complete

## 1. 目标

添加两个便利查询函数：
1. `stdlib_trait_count() -> usize` —— 返回 stdlib 注册表中的 trait 总数
2. `stdlib_all_traits() -> Vec<&'static str>` —— 返回所有 stdlib trait 名称列表

当前 `ALL_REGISTERED_TRAITS` 常量在 `stdlib_traits_with_method` 和
`stdlib_traits_with_vtable` 中各自重复定义。Stage 5.86 提取为模块级
常量 + 两个公共查询函数，消除重复并提供完整列表访问。

## 2. 设计动机

- `stdlib_traits_with_method` 返回**有指定方法的 trait**（filtered）
- `stdlib_traits_with_vtable` 返回**有 vtable 的 trait**（filtered）
- 缺少：返回**所有** stdlib trait 的查询（unfiltered）

`stdlib_all_traits()` 填补这个空缺。`stdlib_trait_count()` 是其便利
包装（`.len()`），与 `stdlib_trait_method_count` 对称。

## 3. 设计

### 3.1 新增 API

```rust
/// Stage 5.86: Return the total number of stdlib traits (marker + method).
pub fn stdlib_trait_count() -> usize

/// Stage 5.86: Return all stdlib trait names (marker + method).
pub fn stdlib_all_traits() -> Vec<&'static str>
```

### 3.2 实现策略

提取模块级常量 `STDLIB_TRAITS: &[&str]`（合并现有的两个本地
`ALL_REGISTERED_TRAITS`），让 `stdlib_traits_with_method` 和
`stdlib_traits_with_vtable` 复用它，消除重复。

### 3.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `stdlib_trait_count` | `<noun>_<noun>_<noun>` | ✅ |
| `stdlib_all_traits` | `<noun>_<adj>_<noun>` | ✅ |

参考 v1.6 (Stage 5.36) 的 `stdlib_trait_method_count` 对称命名。
`stdlib_all_traits` 遵循 Rust API guidelines 的 `all_` 前缀约定
（如 `Vec::all`、`HashMap::all_keys` 等）。

### 3.4 §16 接口隔离

- 输入：无（`()` ）
- 输出：`usize` / `Vec<&'static str>`
- 纯只读，无副作用
- 复用现有 `STDLIB_TRAITS` 常量，无新依赖

### 3.5 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | stdlib_trait_count > 0 | ✓ |
| 2 | stdlib_trait_count >= 30 (markers + method traits) | ✓ |
| 3 | stdlib_all_traits 非空 | ✓ |
| 4 | stdlib_all_traits 包含 "Copy" (marker) | ✓ |
| 5 | stdlib_all_traits 包含 "Clone" (method trait) | ✓ |
| 6 | stdlib_all_traits 包含 "Add" (arithmetic) | ✓ |
| 7 | stdlib_all_traits.len() == stdlib_trait_count() | ✓ |
| 8 | stdlib_all_traits 不包含 "Foo" (user-defined) | ✓ |
| 9 | stdlib_all_traits 不包含 "" (empty) | ✓ |
| 10 | stdlib_all_traits 与 is_stdlib_trait 一致性 | 每个 trait → true |
| 11 | stdlib_all_traits 与 stdlib_traits_with_vtable 关系 | with_vtable ⊆ all |
| 12 | 无副作用——重复调用结果一致 | ✓ |

## 4. 不在本 stage 范围

- ❌ 修改现有查询函数的签名
- ❌ 用户自定义 trait 支持
- ❌ mir/lower 拆分（TD-011, Stage 6）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
