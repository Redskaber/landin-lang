# Stage 5.67 开发计划：emit_dyn_trait_method_call_text

> **阶段**: Stage 5.67
> **版本**: v0.11.62 → v0.11.63
> **状态**: ✅ Complete

## 1. 目标

添加 free function `emit_dyn_trait_method_call_text()`——将
`DynTraitMethodCall`（Stage 5.66 MIR 表示）转换为 vtable 间接调用的
LLVM IR 文本。这是 dyn Trait 方法调用 MIR lowering 的**第一步实质性实现**。

## 2. 设计

### 2.1 新增 API

```rust
pub fn emit_dyn_trait_method_call_text(call: &DynTraitMethodCall) -> String
```

### 2.2 LLVM IR 格式

```text
; dyn <trait>.<type>::<method> (slot=<slot_index>, params=<param_count>)
%vtable_ptr = getelementptr { ptr, ptr }, ptr %dynptr, i32 0, i32 1
%method_fn = load ptr, ptr %vtable_ptr, i32 <slot_index>
%result = call ptr %method_fn(ptr %self, ptr %arg0, ...)
```

### 2.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_dyn_trait_method_call_text` | `<verb>_<noun>_<noun>_<noun>_<noun>` | ✅ |

### 2.4 §16 接口隔离

输入 `&DynTraitMethodCall`，输出 `String`。无循环依赖。

---

**创建日期**: 2026-07-24
