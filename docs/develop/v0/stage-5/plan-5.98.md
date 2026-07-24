# Stage 5.98 开发计划：stdlib_trait_methods_by_is_unsafe — 按 is_unsafe 反向查询

> **阶段**: Stage 5.98
> **版本**: v0.11.93 → v0.11.94
> **状态**: ✅ Complete

## 1. 目标

添加 `stdlib_trait_methods_by_is_unsafe(is_unsafe: bool) -> Vec<(&'static str, &'static str)>` ——
返回所有 unsafe 标志匹配的 stdlib trait 方法。

与 5.95 的 `by_self_kind` 和 5.96 的 `by_return_kind` 对称，完成反向查询系列。

## 2. 设计

### 2.1 新增 API

```rust
pub fn stdlib_trait_methods_by_is_unsafe(
    is_unsafe: bool,
) -> Vec<(&'static str, &'static str)>
```

### 2.2 命名标准化

`stdlib_trait_methods_by_is_unsafe` — `<noun>_<noun>_<noun>_<prep>_<is_adj>` (plural) ✅

### 2.3 §16 接口隔离

纯只读，复用 `STDLIB_TRAITS` + `stdlib_trait_methods`，无新依赖。

### 2.4 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | is_unsafe=false 返回非空（所有当前方法都是 safe） | ✓ |
| 2 | is_unsafe=true 返回空（当前无 unsafe 方法） | ✓ |
| 3 | is_unsafe=false 包含 ("Clone", "clone") | ✓ |
| 4 | is_unsafe=false 包含 ("Drop", "drop") | ✓ |
| 5 | 所有返回的方法 is_unsafe 匹配 | ✓ |
| 6 | false + true 覆盖所有方法 | ✓ |
| 7 | 无副作用 | ✓ |

---

**创建日期**: 2026-07-24
