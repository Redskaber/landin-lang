# Stage 5.37 开发计划：stdlib vtable slot layout

> **阶段**: Stage 5.37
> **版本**: v0.11.32 → v0.11.33
> **状态**: ✅ Complete

## 1. 目标

在 Stage 5.36（trait 方法签名注册表）基础上，为每个 stdlib trait 计算**确定性 vtable slot 布局**：
将 trait 的方法按 `stdlib_trait_methods()` 返回顺序映射为 0-based 的 vtable slot index。
这是 dyn Trait codegen 的最后一块静态基础设施——codegen 在 emit `@.vtable.<trait>.<type>`
全局时需要知道每个方法在 vtable 中的字节偏移（slot_index × pointer_size），以及
vtable 的总 slot 数（决定全局的 element count）。

这一步**仅添加查询 API**，不修改 codegen（dyn Trait MIR lowering 在 Stage 5.38+）。
保持增量式开发，每个 stage 都可独立审查。

## 2. 设计

### 2.1 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `stdlib_trait_method_index` | `(trait_name, method_name) -> Option<u32>` | 查询单个方法的 vtable slot |
| `stdlib_vtable_layout` | `(trait_name) -> Option<Vec<StdlibVtableSlot>>` | 完整 slot 布局（slot_index + method_ref） |
| `stdlib_vtable_slot_count` | `(trait_name) -> Option<u32>` | vtable 总 slot 数 |
| `is_stdlib_marker_trait` | `(trait_name) -> bool` | 是否为 marker trait（0 slot） |
| `stdlib_traits_with_vtable` | `() -> Vec<&'static str>` | 所有有 vtable（≥1 slot）的 trait |

### 2.2 新增类型

```rust
/// 一个 vtable slot 的描述：方法索引 + 方法引用。
pub struct StdlibVtableSlot {
    pub slot_index: u32,
    pub method: &'static StdlibTraitMethod,
}
```

### 2.3 vtable slot 编号规则

- slot index 从 0 开始，按 `stdlib_trait_methods(trait_name)` 返回的 slice 顺序递增
- marker traits (Copy/Send/Sync/Sized/Unpin/Eq) slot_count = 0
- 未注册 trait 返回 `None`
- 同一 trait 的 slot 布局在进程内**确定性**（不依赖 HashMap 迭代顺序）

### 2.4 命名标准化（§23）

| API/类型 | 命名规则 | 合规 |
|----------|---------|------|
| `StdlibVtableSlot` | `<Noun><Noun><Noun>` | ✅ |
| `stdlib_trait_method_index` | `<noun>_<noun>_<noun>_<noun>` | ✅ |
| `stdlib_vtable_layout` | `<noun>_<noun>_<noun>` | ✅ |
| `stdlib_vtable_slot_count` | `<noun>_<noun>_<noun>_<noun>` | ✅ |
| `is_stdlib_marker_trait` | `is_<noun>_<adj>_<noun>` | ✅ |
| `stdlib_traits_with_vtable` | `<noun>_<noun>_with_<noun>` | ✅ |
| `slot_index` (field) | `<noun>_<noun>` | ✅ |
| `method` (field) | `<noun>` | ✅ |

### 2.5 §16 接口隔离

`StdlibVtableSlot` 包含 `&'static StdlibTraitMethod`（已有，stdlib 内部），
不引用 `mir::ty` / `codegen::EmitType`，无循环依赖。所有查询函数是纯函数，
输入 `&str`，输出 `Option<...>`，可在 driver/typeck/codegen 任一阶段调用。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1130 + 新增 ~12 = ~1142）
4. §1.2 交付前验收：全绿

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_stdlib_trait_method_index_clone` | Clone::clone → 0, Clone::clone_from → 1 |
| `test_stdlib_trait_method_index_drop` | Drop::drop → 0 |
| `test_stdlib_trait_method_index_partial_eq` | PartialEq::eq → 0, ne → 1 |
| `test_stdlib_trait_method_index_add` | Add::add → 0 |
| `test_stdlib_trait_method_index_unknown_trait` | 未注册 trait → None |
| `test_stdlib_trait_method_index_unknown_method` | Clone::bogus → None |
| `test_stdlib_trait_method_index_marker` | Copy::clone → None (marker 无 slot) |
| `test_stdlib_vtable_layout_clone` | Clone 布局有 2 个 slot |
| `test_stdlib_vtable_layout_deterministic` | 同一 trait 两次查询返回相同顺序 |
| `test_stdlib_vtable_slot_count` | Clone=2, Drop=1, Copy=0 |
| `test_is_stdlib_marker_trait_true` | Copy/Send/Sync/Sized/Unpin/Eq |
| `test_is_stdlib_marker_trait_false` | Clone/Drop/Add 等 |
| `test_is_stdlib_marker_trait_unknown` | 未注册 trait → false |
| `test_stdlib_traits_with_vtable_excludes_markers` | 不含 Copy/Send/... |
| `test_stdlib_traits_with_vtable_includes_clone` | 含 Clone |
| `test_stdlib_vtable_slot_struct` | StdlibVtableSlot 字段访问 |

## 5. 后续依赖

- **Stage 5.38+ (dyn Trait MIR lowering)**: codegen 调用 `stdlib_vtable_layout()`
  发射 `@.vtable.<trait>.<type>` 全局时确定 element count；调用
  `stdlib_trait_method_index()` 计算 method 调用的字节偏移。
- **Stage 5.39+ (typeck trait bound solving)**: 验证 dyn Trait method 调用的
  slot_index 在 vtable 范围内。

---

**创建日期**: 2026-07-23
