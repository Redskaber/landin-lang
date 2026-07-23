# Stage 5.61 开发计划：DynTraitFatPtr MIR-level representation

> **阶段**: Stage 5.61
> **版本**: v0.11.56 → v0.11.57
> **状态**: ✅ Complete

## 1. 目标

**开始 dyn Trait MIR lowering**——Stage 5 的核心目标。第一步：添加
MIR 级别的 `DynTraitFatPtr` 结构体，表示 `dyn Trait` 值的 (data, vtable)
fat pointer 对。这是纯数据类型，不修改现有 lowering 路径——为 Stage 5.62+
的实际 MIR lowering 逻辑做准备。

## 2. 设计

### 2.1 新增类型

```rust
/// MIR-level representation of a `dyn Trait` fat pointer value.
///
/// A `dyn Trait` value is a fat pointer: (data_ptr, vtable_ptr).
/// This struct captures both components at the MIR level, before
/// they are lowered to LLVM IR.
pub struct DynTraitFatPtr {
    pub trait_name: String,
    pub type_name: String,
    pub data_symbol: String,
    pub vtable_symbol: String,
    pub dynptr_symbol: String,
}
```

### 2.2 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `DynTraitFatPtr::new` | `(trait_name, type_name) -> Self` | 构造 |
| `DynTraitFatPtr::is_marker` | `&self -> bool` | 是否 marker trait（无方法） |

### 2.3 命名标准化（§23）

| 类型 | 命名规则 | 合规 |
|------|---------|------|
| `DynTraitFatPtr` | `<Noun><Noun><Noun>` | ✅ |

字段命名：`trait_name` / `type_name` / `data_symbol` / `vtable_symbol` / `dynptr_symbol` — 全部 `<noun>_<noun>` ✅

### 2.4 §16 接口隔离

`DynTraitFatPtr` 仅依赖 `String`，不引用 `mir::ty` / `codegen::EmitType` /
`traits::TraitResolver`，无循环依赖。

### 2.5 放置位置

`src/mir/dyn_trait.rs`——MIR 模块下的新文件，与其他 MIR 类型（ty.rs / place.rs / body.rs）并列。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1442 + 新增 ~8 = ~1450）
4. §1.2 交付前验收：全绿

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_dyn_trait_fat_ptr_new` | 构造 |
| `test_dyn_trait_fat_ptr_fields` | 字段访问 |
| `test_dyn_trait_fat_ptr_is_marker_false` | 非 marker |
| `test_dyn_trait_fat_ptr_eq` | PartialEq/Eq 派生 |
| `test_dyn_trait_fat_ptr_clone` | Clone 派生 |
| `test_dyn_trait_fat_ptr_debug` | Debug 派生 |
| `test_dyn_trait_fat_ptr_real_scenario` | 模拟真实场景 |
| `test_dyn_trait_fat_ptr_multiple` | 多个 fat ptr |

---

**创建日期**: 2026-07-23
