# Stage 5.38 开发计划：stdlib vtable byte size + pointer-width-aware layout

> **阶段**: Stage 5.38
> **版本**: v0.11.33 → v0.11.34
> **状态**: ✅ Complete

## 1. 目标

在 Stage 5.37（vtable slot 布局）基础上，添加**指针宽度感知**的 vtable 字节
大小计算 API。codegen 在发射 `@.vtable.<trait>.<type>` 全局时需要知道：

1. 全局的 LLVM 类型 `[n x ptr]` 中的 `n`（已有：`stdlib_vtable_slot_count`）
2. 全局的字节大小（`n × pointer_width`）—— 用于 `alloca` / `getelementptr` 计算
3. 单个方法调用的字节偏移（`slot_index × pointer_width`）—— 用于 `getelementptr i8, ptr @vtable, i64 offset`

这一步把"slot index"翻译为"byte offset"，是 codegen 真正能直接使用的形态。

## 2. 设计

### 2.1 新增类型

```rust
/// 指针宽度枚举 —— 决定 vtable 中每个 slot 的字节大小。
pub enum StdlibPointerWidth {
    Pointer32,  // 4 bytes/slot (32-bit target)
    Pointer64,  // 8 bytes/slot (64-bit target)
}
```

### 2.2 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `StdlibPointerWidth::byte_size` | `method -> u32` | 单 slot 字节大小 |
| `stdlib_pointer_width_bytes` | `(width) -> u32` | free fn 形式 |
| `stdlib_vtable_byte_size` | `(trait_name, width) -> Option<u64>` | 整个 vtable 字节大小 |
| `stdlib_vtable_method_offset` | `(trait_name, method_name, width) -> Option<u64>` | 方法在 vtable 中的字节偏移 |

### 2.3 计算规则

- `vtable_byte_size = slot_count × pointer_width_bytes`
- `method_offset = slot_index × pointer_width_bytes`
- marker traits (`Copy`/`Send`/...) → byte_size = `Some(0)`, method_offset = `None`
- 未注册 trait → byte_size = `None`, method_offset = `None`
- 已知 trait + 未知方法 → method_offset = `None`

### 2.4 命名标准化（§23）

| API/类型 | 命名规则 | 合规 |
|----------|---------|------|
| `StdlibPointerWidth` | `<Noun><Noun><Noun>` | ✅ |
| `Pointer32` / `Pointer64` | `<Noun><Digits>` (变体) | ✅ |
| `byte_size` (method) | `<noun>_<noun>` | ✅ |
| `stdlib_pointer_width_bytes` | `<noun>_<noun>_<noun>_<noun>` | ✅ |
| `stdlib_vtable_byte_size` | `<noun>_<noun>_<noun>_<noun>` | ✅ |
| `stdlib_vtable_method_offset` | `<noun>_<noun>_<noun>_<noun>` | ✅ |

### 2.5 §16 接口隔离

所有新 API 仅依赖 `StdlibPointerWidth`（stdlib 内部枚举）+ 已有
`stdlib_vtable_slot_count` / `stdlib_trait_method_index`。不引用
`codegen::EmitType` 或 `mir::ty`，无循环依赖。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1152 + 新增 ~12 = ~1164）
4. §1.2 交付前验收：全绿

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_stdlib_pointer_width_byte_size_32` | Pointer32 → 4 |
| `test_stdlib_pointer_width_byte_size_64` | Pointer64 → 8 |
| `test_stdlib_pointer_width_bytes_free_fn` | free fn 形式 |
| `test_stdlib_vtable_byte_size_clone_32` | Clone@32bit → 8 (2×4) |
| `test_stdlib_vtable_byte_size_clone_64` | Clone@64bit → 16 (2×8) |
| `test_stdlib_vtable_byte_size_drop` | Drop → 4/8 |
| `test_stdlib_vtable_byte_size_marker` | Copy → Some(0) |
| `test_stdlib_vtable_byte_size_unknown` | BogusTrait → None |
| `test_stdlib_vtable_method_offset_clone` | Clone::clone@0, clone_from@offset=width |
| `test_stdlib_vtable_method_offset_drop` | Drop::drop@0 |
| `test_stdlib_vtable_method_offset_partial_eq_64` | eq@0, ne@8 |
| `test_stdlib_vtable_method_offset_marker` | Copy::clone → None |
| `test_stdlib_vtable_method_offset_unknown_method` | Clone::bogus → None |
| `test_stdlib_vtable_method_offset_unknown_trait` | Bogus::x → None |

## 5. 后续依赖

- **Stage 5.39+ (dyn Trait MIR lowering)**: codegen 直接调用
  `stdlib_vtable_byte_size()` 决定 `alloca` 大小；调用
  `stdlib_vtable_method_offset()` 生成 `getelementptr i8, ptr @vtable, i64 offset`。
- **Stage 5.40+ (dyn Trait typeck)**: 验证 method_offset 在 vtable_byte_size 范围内。

---

**创建日期**: 2026-07-23
