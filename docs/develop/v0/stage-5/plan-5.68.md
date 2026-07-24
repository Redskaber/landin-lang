# Stage 5.68 开发计划：build_dyn_trait_method_calls_from_fat_ptrs

> **阶段**: Stage 5.68
> **版本**: v0.11.63 → v0.11.64
> **状态**: ✅ Complete

## 1. 目标

添加 free function `build_dyn_trait_method_calls_from_fat_ptrs()`——从
`&[DynTraitFatPtr]` + stdlib trait method index 构造
`Vec<DynTraitMethodCall>`。这是 stdlib 查询层（Stage 5.36-5.37）与
MIR 方法调用表示（Stage 5.66）之间的**桥接函数**。

## 2. 设计

### 2.1 新增 API

```rust
/// 从 fat ptrs + stdlib trait method index 构造 DynTraitMethodCall 列表。
pub fn build_dyn_trait_method_calls_from_fat_ptrs(
    fat_ptrs: &[DynTraitFatPtr],
) -> Vec<DynTraitMethodCall>
```

### 2.2 计算规则

对每个 `fat_ptr`：
1. `stdlib_trait_methods(&fat_ptr.trait_name)` 获取方法列表（Stage 5.36）
2. 对每个方法，`stdlib_trait_method_index(&fat_ptr.trait_name, &method.name)` 获取 slot index（Stage 5.37）
3. 构造 `DynTraitMethodCall::from_fat_ptr(fat_ptr, method.name, slot_index, method.param_count)`
4. 跳过未注册的 trait（返回 None 的）

### 2.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `build_dyn_trait_method_calls_from_fat_ptrs` | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` | ✅ |

### 2.4 §16 接口隔离

输入 `&[DynTraitFatPtr]`，输出 `Vec<DynTraitMethodCall>`。调用
`stdlib::stdlib_trait_methods()` + `stdlib::stdlib_trait_method_index()`
（单向：mir → stdlib，无循环依赖）。

---

**创建日期**: 2026-07-24
