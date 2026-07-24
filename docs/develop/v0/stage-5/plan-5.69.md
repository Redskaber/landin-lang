# Stage 5.69 开发计划：emit_dyn_trait_method_calls_text_batch

> **阶段**: Stage 5.69
> **版本**: v0.11.64 → v0.11.65
> **状态**: ✅ Complete

## 1. 目标

添加 free function `emit_dyn_trait_method_calls_text_batch()`——批量将
`&[DynTraitMethodCall]` 转换为 `Vec<String>`（所有方法调用的 LLVM IR 文本）。
这是 Stage 5.67 `emit_dyn_trait_method_call_text()` 的批量版本。

## 2. 设计

### 2.1 新增 API

```rust
pub fn emit_dyn_trait_method_calls_text_batch(calls: &[DynTraitMethodCall]) -> Vec<String>
```

### 2.2 计算规则

对每个 `call` 调用 `emit_dyn_trait_method_call_text(call)` (Stage 5.67)，收集结果。

### 2.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_dyn_trait_method_calls_text_batch` | `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` | ✅ |

### 2.4 §16 接口隔离

输入 `&[DynTraitMethodCall]`，输出 `Vec<String>`。无循环依赖。

---

**创建日期**: 2026-07-24
