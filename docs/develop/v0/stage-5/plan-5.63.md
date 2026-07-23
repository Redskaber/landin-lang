# Stage 5.63 开发计划：emit_dyn_trait_fat_ptr_text

> **阶段**: Stage 5.63
> **版本**: v0.11.58 → v0.11.59
> **状态**: ✅ Complete

## 1. 目标

添加 free function `emit_dyn_trait_fat_ptr_text()`——将
`DynTraitFatPtr`（Stage 5.61 MIR 表示）转换为 LLVM IR 文本（String）。
这是 MIR 表示与 codegen 输出之间的**转换函数**，为 Stage 5.64+
的实际 MIR lowering 逻辑做准备。

## 2. 设计

### 2.1 新增 API

```rust
/// 将 DynTraitFatPtr 转换为 LLVM IR 文本。
pub fn emit_dyn_trait_fat_ptr_text(fat_ptr: &DynTraitFatPtr) -> String
```

### 2.2 计算规则

调用 `crate::codegen::emit_dynptr_global_text(fat_ptr.dynptr_symbol,
fat_ptr.data_symbol, fat_ptr.vtable_symbol)` (Stage 5.48)。

### 2.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_dyn_trait_fat_ptr_text` | `<verb>_<noun>_<noun>_<noun>_<noun>` | ✅ |

### 2.4 §16 接口隔离

输入 `&DynTraitFatPtr`，输出 `String`。调用 `codegen::emit_dynptr_global_text()`
（跨模块但单向：mir → codegen，无循环依赖）。

---

**创建日期**: 2026-07-23
