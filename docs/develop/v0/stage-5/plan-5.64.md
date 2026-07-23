# Stage 5.64 开发计划：emit_dyn_trait_fat_ptrs_text_batch

> **阶段**: Stage 5.64
> **版本**: v0.11.59 → v0.11.60
> **状态**: ✅ Complete

## 1. 目标

添加 free function `emit_dyn_trait_fat_ptrs_text_batch()`——Stage 5.63
`emit_dyn_trait_fat_ptr_text()` 的批量版本。输入 `&[DynTraitFatPtr]`，
输出 `Vec<String>`——所有 fat ptr 的 LLVM IR 文本。

## 2. 设计

### 2.1 新增 API

```rust
/// 批量将 DynTraitFatPtr 列表转换为 LLVM IR 文本。
pub fn emit_dyn_trait_fat_ptrs_text_batch(fat_ptrs: &[DynTraitFatPtr]) -> Vec<String>
```

### 2.2 计算规则

对每个 `fat_ptr` 调用 `emit_dyn_trait_fat_ptr_text(fat_ptr)` (Stage 5.63)，收集结果。

### 2.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_dyn_trait_fat_ptrs_text_batch` | `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` | ✅ |

### 2.4 §16 接口隔离

输入 `&[DynTraitFatPtr]`，输出 `Vec<String>`。无循环依赖。

---

**创建日期**: 2026-07-23
