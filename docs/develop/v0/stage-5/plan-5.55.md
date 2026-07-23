# Stage 5.55 开发计划：codegen trait-dispatch emission text batch (plan-based)

> **阶段**: Stage 5.55
> **版本**: v0.11.50 → v0.11.51
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function
`emit_trait_dispatch_globals_text_batch()`：输入
`&CodegenTraitDispatchEmissionPlan`，输出 `Vec<String>`——所有 vtable +
dynptr 全局的 LLVM IR 文本，**无需 Emitter trait**。这是 Stage 5.45
`emit_vtable_globals_batch()` 的 plan-based 对应版本，扩展到 vtable + dynptr。

这使得 text-based 批量生成无需 Emitter trait，对测试和未来 codegen 路径
（可直接 push 预格式化文本）有用。

## 2. 设计

### 2.1 新增 API

```rust
/// 从 plan 生成所有 trait-dispatch globals 的 LLVM IR 文本（无需 Emitter）。
pub fn emit_trait_dispatch_globals_text_batch(
    plan: &CodegenTraitDispatchEmissionPlan,
) -> Vec<String>
```

### 2.2 计算规则

1. 对每个 `plan.vtable_specs`，调用 `emit_vtable_global_text(spec.global_name, spec.method_symbols)` (Stage 5.44)
2. 对每个 `plan.dynptr_specs`，调用 `emit_dynptr_global_text(spec.global_name, spec.data_symbol, spec.vtable_symbol)` (Stage 5.48)
3. 收集所有 IR 文本到 `Vec<String>`

### 2.3 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_trait_dispatch_globals_text_batch` | `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` | ✅ |

`emit_` 前缀一致。`_text_batch` 表明返回 LLVM IR 文本批量（无需 Emitter）。

### 2.4 §16 接口隔离

输入 `&CodegenTraitDispatchEmissionPlan`，输出 `Vec<String>`。不引用
`mir::ty` / `Emitter` / `TraitResolver` / `Rodeo`，无循环依赖。

### 2.5 不修改现有路径

- 所有现有 codegen 函数保持不变
- Stage 5.56 才让 codegen 调用这个 text batch

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1384 + 新增 ~12 = ~1396）
4. §1.2 交付前验收：全绿
5. 输出与 `emit_trait_dispatch_globals_from_plan()` 生成的 IR **逐字节一致**

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_emit_trait_dispatch_globals_text_batch_empty` | 空 plan → 空 Vec |
| `test_emit_trait_dispatch_globals_text_batch_single` | 单 spec |
| `test_emit_trait_dispatch_globals_text_batch_multi` | 多 spec |
| `test_emit_trait_dispatch_globals_text_batch_match_orchestrator` | **== emit_trait_dispatch_globals_from_plan** |
| `test_emit_trait_dispatch_globals_text_batch_no_side_effects` | 纯函数 |
| `test_emit_trait_dispatch_globals_text_batch_vtable_lines` | vtable IR 行 |
| `test_emit_trait_dispatch_globals_text_batch_dynptr_lines` | dynptr IR 行 |
| `test_emit_trait_dispatch_globals_text_batch_count_matches` | 行数 == 2 × specs |
| `test_emit_trait_dispatch_globals_text_batch_order` | vtable 在 dynptr 前 |
| `test_emit_trait_dispatch_globals_text_batch_real_scenario` | 模拟真实场景 |
| `test_emit_trait_dispatch_globals_text_batch_no_emitter_needed` | 无需 Emitter |
| `test_emit_trait_dispatch_globals_text_batch_deterministic` | 重复调用相同结果 |

## 5. 后续依赖

- **Stage 5.56 (codegen trait-dispatch emission refactor)**:
  - codegen 可直接 push text batch 到 emitter.globals
  - TextEmitter + emit_*() 委托给 free fn
- **Stage 5.57+ (dyn Trait MIR lowering)**: 直接调用 text batch

---

**创建日期**: 2026-07-23
