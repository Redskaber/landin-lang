# Stage 5.74 开发计划：emit_dyn_trait_mir_plan_text

> **阶段**: Stage 5.74
> **版本**: v0.11.69 → v0.11.70
> **状态**: ✅ Complete

## 1. 目标

添加 free function `emit_dyn_trait_mir_plan_text()`——将 `DynTraitMIRPlan`
(Stage 5.73) 转换为完整的 LLVM IR 文本：所有 fat ptr 全局定义 + 所有方法调用
IR + 汇总注释。这是 dyn Trait MIR lowering 的**完整 IR 文本生成器**——
一次调用获取整个项目的 dyn Trait LLVM IR。

## 2. 设计

### 2.1 新增 API

```rust
pub fn emit_dyn_trait_mir_plan_text(plan: &DynTraitMIRPlan) -> String
```

### 2.2 计算规则

1. 汇总注释行：`; DynTraitMIRSummary: N fat ptrs, M method calls, K slots`
2. 对每个 `plan.fat_ptrs`，调用 `emit_dyn_trait_fat_ptr_text()` (Stage 5.63)
3. 对每个 `plan.method_calls`，调用 `emit_dyn_trait_method_call_text()` (Stage 5.67)
4. 用空行分隔各部分

### 2.3 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_dyn_trait_mir_plan_text` | `<verb>_<noun>_<noun>_<noun>_<noun>` | ✅ |

### 2.4 §16 接口隔离

输入 `&DynTraitMIRPlan`，输出 `String`。无循环依赖。

---

**创建日期**: 2026-07-24
