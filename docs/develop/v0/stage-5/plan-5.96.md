# Stage 5.96 开发计划：stdlib_trait_methods_by_return_kind — 按 return_kind 反向查询

> **阶段**: Stage 5.96
> **版本**: v0.11.91 → v0.11.92
> **状态**: ✅ Complete

## 1. 目标

添加 `stdlib_trait_methods_by_return_kind(kind) -> Vec<(&'static str, &'static str)>` ——
返回所有具有指定 return_kind 的 stdlib trait 方法。

与 5.95 的 `stdlib_trait_methods_by_self_kind` 对称，提供按返回类型反向查询。

## 2. 设计

### 2.1 新增 API

```rust
pub fn stdlib_trait_methods_by_return_kind(
    kind: StdlibTypeKind,
) -> Vec<(&'static str, &'static str)>
```

### 2.2 命名标准化

`stdlib_trait_methods_by_return_kind` — `<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` (plural)
✅ 与 `stdlib_trait_methods_by_self_kind` (5.95) 同家族。

### 2.3 §16 接口隔离

纯只读，复用 `STDLIB_TRAITS` + `stdlib_trait_methods`，无新依赖。

### 2.4 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | Unit 返回非空 (Drop::drop, clone_from 等) | ✓ |
| 2 | Bool 返回非空 (PartialEq::eq/ne) | ✓ |
| 3 | AllocType 返回非空 (Clone::clone, Default::default 等) | ✓ |
| 4 | StdType 返回非空 (Display::fmt, PartialOrd::partial_cmp 等) | ✓ |
| 5 | Unit 包含 ("Drop", "drop") | ✓ |
| 6 | Bool 包含 ("PartialEq", "eq") | ✓ |
| 7 | 所有返回的方法 return_kind 匹配 | ✓ |
| 8 | 无副作用 | ✓ |
| 9 | 所有 return_kind 覆盖所有方法 | ✓ |

---

**创建日期**: 2026-07-24
