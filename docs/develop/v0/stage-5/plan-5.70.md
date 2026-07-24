# Stage 5.70 开发计划：emit_dyn_trait_method_calls_text_batch_from_resolver

> **阶段**: Stage 5.70
> **版本**: v0.11.65 → v0.11.66
> **状态**: ✅ Complete

## 1. 目标

添加 convenience entry point `emit_dyn_trait_method_calls_text_batch_from_resolver()`——
一次调用从 `(&TraitResolver, &Rodeo)` 到 `Vec<String>`（所有 dyn Trait 方法调用的
LLVM IR 文本）。组合 Stage 5.62 + 5.68 + 5.69。

## 2. 设计

### 2.1 新增 API

```rust
pub fn emit_dyn_trait_method_calls_text_batch_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &lasso::Rodeo,
) -> Vec<String>
```

### 2.2 计算规则

1. `let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(trait_resolver, interner);` (Stage 5.62)
2. `let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fat_ptrs);` (Stage 5.68)
3. `emit_dyn_trait_method_calls_text_batch(&calls)` (Stage 5.69)

### 2.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_dyn_trait_method_calls_text_batch_from_resolver` | `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>_<prep>_<noun>` | ✅ |

### 2.4 §16 接口隔离

输入 `&TraitResolver` + `&Rodeo`，输出 `Vec<String>`。无循环依赖。

---

**创建日期**: 2026-07-24
