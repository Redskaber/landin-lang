# Stage 5.85 开发计划：is_stdlib_trait — trait 级别成员查询

> **阶段**: Stage 5.85
> **版本**: v0.11.80 → v0.11.81
> **状态**: ✅ Complete

## 1. 目标

添加 free function `is_stdlib_trait(trait_name: &str) -> bool` —— 检查
trait 名称是否在 stdlib 注册表中（包括 marker traits 和有方法的 traits）。

这是对现有 `is_stdlib_marker_trait`（仅 marker）和 `is_stdlib_trait_method`
（方法级查询）的补充，提供**trait 级别的完整成员查询**。

## 2. 设计动机

当前 stdlib 有：
- `is_stdlib_marker_trait(name) -> bool` —— 仅检查 marker traits (Copy/Send/Sync/Sized/Unpin/Eq)
- `is_stdlib_trait_method(trait, method) -> bool` —— 检查 (trait, method) 对
- `stdlib_trait_methods(trait) -> Option<&[...]>` —— 返回方法列表（marker 返回 Some(&[])）
- `stdlib_traits_with_vtable() -> Vec<&str>` —— 返回有 vtable 的 trait 列表

**缺失**：一个简单的 `is_stdlib_trait(name) -> bool` 来检查**任何** stdlib trait
（marker + 有方法的）。当前需要组合 `is_stdlib_marker_trait` + `stdlib_trait_methods.is_some()`
才能判断，不直观。

## 3. 设计

### 3.1 新增 API

```rust
/// Stage 5.85: Check if a trait name is a stdlib trait (marker or with methods).
pub fn is_stdlib_trait(trait_name: &str) -> bool
```

### 3.2 计算规则

1. 如果 `is_stdlib_marker_trait(trait_name)` 返回 true → true
2. 否则如果 `stdlib_trait_methods(trait_name).is_some()` → true
3. 否则 → false

### 3.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `is_stdlib_trait` | `is_<noun>_<noun>` | ✅ |

参考 §8.1 helper-verb `is_` 前缀 + v1.6 (Stage 5.36) 的
`is_stdlib_marker_trait` / `is_stdlib_trait_method` 同家族。

### 3.4 §16 接口隔离

- 输入：`&str`
- 输出：`bool`
- 纯只读，无副作用
- 复用现有 `is_stdlib_marker_trait` + `stdlib_trait_methods`，无新依赖

### 3.5 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | "Copy" (marker) | true |
| 2 | "Send" (marker) | true |
| 3 | "Clone" (有方法) | true |
| 4 | "Drop" (有方法) | true |
| 5 | "Display" (有方法) | true |
| 6 | "Add" (有方法) | true |
| 7 | "Foo" (用户自定义) | false |
| 8 | "" (空字符串) | false |
| 9 | "clone" (方法名，非 trait) | false |
| 10 | 大小写敏感："clone" ≠ "Clone" | false for "clone" |
| 11 | 与 is_stdlib_marker_trait 一致性 | marker → true |
| 12 | 与 stdlib_trait_methods 一致性 | some → true |

## 4. 不在本 stage 范围

- ❌ 用户自定义 trait 的 dyn 支持
- ❌ 修改现有查询函数
- ❌ mir/lower 拆分（TD-011, Stage 6）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
