# Stage 5.66 开发计划：DynTraitMethodCall MIR representation

> **阶段**: Stage 5.66
> **版本**: v0.11.61 → v0.11.62
> **状态**: ✅ Complete

## 1. 目标

添加 MIR 级别的 `DynTraitMethodCall` 结构体，表示 `dyn Trait` 方法调用的
全部信息：接收者 fat pointer + 方法名 + vtable slot index + 参数数量。
这是 dyn Trait method call MIR lowering 的**最后一块基础设施**——Stage 5.67+
即可开始在 MIR lowering 中实际处理 dyn Trait 方法调用。

## 2. 设计

### 2.1 新增类型

```rust
/// MIR-level representation of a `dyn Trait` method call.
pub struct DynTraitMethodCall {
    pub trait_name: String,
    pub type_name: String,
    pub method_name: String,
    pub slot_index: u32,
    pub param_count: u32,
}
```

### 2.2 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `DynTraitMethodCall::new` | `(trait_name, type_name, method_name, slot_index, param_count) -> Self` | 构造 |
| `DynTraitMethodCall::from_fat_ptr` | `(&DynTraitFatPtr, method_name, slot_index, param_count) -> Self` | 从 fat ptr 构造 |

### 2.3 命名标准化

| 类型 | 命名规则 | 合规 |
|------|---------|------|
| `DynTraitMethodCall` | `<Noun><Noun><Noun>` | ✅ |

### 2.4 §16 接口隔离

仅依赖 `String` + `&DynTraitFatPtr`（同模块），无循环依赖。

---

**创建日期**: 2026-07-23
