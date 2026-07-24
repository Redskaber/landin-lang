# Stage 5.99 开发计划：stdlib_trait_methods_by_param_count — 按 param_count 反向查询 + Stage 5 收尾

> **阶段**: Stage 5.99（Stage 5 最终子阶段）
> **版本**: v0.11.94 → v0.11.95
> **状态**: ✅ Complete

## 1. 目标

添加 `stdlib_trait_methods_by_param_count(count: u32) -> Vec<(&'static str, &'static str)>` ——
返回所有具有指定参数数量的 stdlib trait 方法。

这是反向查询系列的**第四个也是最后一个维度**，完成所有可查询字段的反向查询覆盖。

## 2. 反向查询系列总结

| Stage | Query | Field |
|-------|-------|-------|
| 5.95 | stdlib_trait_methods_by_self_kind | self_kind |
| 5.96 | stdlib_trait_methods_by_return_kind | return_kind |
| 5.98 | stdlib_trait_methods_by_is_unsafe | is_unsafe |
| **5.99** | **stdlib_trait_methods_by_param_count** | **param_count** |

4 个维度的反向查询全部覆盖。`name` 是查询参数不需要反向查询。

## 3. 设计

### 3.1 新增 API

```rust
pub fn stdlib_trait_methods_by_param_count(
    param_count: u32,
) -> Vec<(&'static str, &'static str)>
```

### 3.2 命名标准化

`stdlib_trait_methods_by_param_count` — `<noun>×3_<prep>_<noun>×2` (plural) ✅

### 3.3 §16 接口隔离

纯只读，复用 `STDLIB_TRAITS` + `stdlib_trait_methods`，无新依赖。

### 3.4 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | param_count=0 返回非空 (Drop::drop, Clone::clone 等) | ✓ |
| 2 | param_count=1 返回非空 (Display::fmt, PartialEq::eq 等) | ✓ |
| 3 | param_count=0 包含 ("Drop", "drop") | ✓ |
| 4 | param_count=1 包含 ("Display", "fmt") | ✓ |
| 5 | param_count=99 返回空 (无方法有 99 参数) | ✓ |
| 6 | 所有返回的方法 param_count 匹配 | ✓ |
| 7 | 无副作用 | ✓ |

---

**创建日期**: 2026-07-24
