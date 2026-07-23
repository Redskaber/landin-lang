# Stage 5.62 开发计划：build_dyn_trait_fat_ptrs_from_resolver

> **阶段**: Stage 5.62
> **版本**: v0.11.57 → v0.11.58
> **状态**: ✅ Complete

## 1. 目标

添加 free function `build_dyn_trait_fat_ptrs_from_resolver()`——从
`TraitResolver.vtables` 构造 `Vec<DynTraitFatPtr>`。这是 Stage 5.61 的
`DynTraitFatPtr`（MIR 表示）与 `TraitResolver`（trait 实现数据源）之间的
**桥接函数**，为 Stage 5.63+ 的实际 MIR lowering 逻辑做准备。

## 2. 设计

### 2.1 新增 API

```rust
/// 从 TraitResolver.vtables 构造 DynTraitFatPtr 列表。
pub fn build_dyn_trait_fat_ptrs_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> Vec<DynTraitFatPtr>
```

### 2.2 计算规则

对每个 `(trait_name, self_ty_name)` key in `trait_resolver.vtables`：
- `trait_str = interner.try_resolve(trait_name).unwrap_or("Trait")`
- `type_str = interner.try_resolve(self_ty_name).unwrap_or("Type")`
- `DynTraitFatPtr::new(trait_str, type_str)`

### 2.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `build_dyn_trait_fat_ptrs_from_resolver` | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` | ✅ |

### 2.4 §16 接口隔离

输入 `&TraitResolver` + `&Rodeo`，输出 `Vec<DynTraitFatPtr>`。无循环依赖。

### 2.5 放置位置

`src/mir/dyn_trait.rs`——与 `DynTraitFatPtr` 定义同文件。

---

**创建日期**: 2026-07-23
