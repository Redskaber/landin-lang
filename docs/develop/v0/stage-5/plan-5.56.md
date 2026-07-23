# Stage 5.56 开发计划：codegen trait-dispatch emission text batch from resolver

> **阶段**: Stage 5.56
> **版本**: v0.11.51 → v0.11.52
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function
`emit_trait_dispatch_globals_text_batch_from_resolver()`：输入
`(&TraitResolver, &Rodeo)`，输出 `Vec<String>`——**一次调用**完成
plan-building + text-batch 生成。这是 codegen 获取所有 trait-dispatch
全局 IR 文本的**便捷入口点**（无需 Emitter + 无需单独 plan 步骤），
是最终委托前的最后一块拼图。

## 2. 设计

### 2.1 新增 API

```rust
/// 从 TraitResolver 一次调用生成所有 trait-dispatch globals 的 LLVM IR 文本。
pub fn emit_trait_dispatch_globals_text_batch_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> Vec<String>
```

### 2.2 计算规则

1. `let plan = build_trait_dispatch_emission_plan(trait_resolver, interner);` (Stage 5.53)
2. `emit_trait_dispatch_globals_text_batch(&plan)` (Stage 5.55)

### 2.3 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_trait_dispatch_globals_text_batch_from_resolver` | `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>_<prep>_<noun>` | ✅ |

`emit_` 前缀一致。`_text_batch` 表明返回 LLVM IR 文本批量。`_from_resolver`
表明输入来自 TraitResolver（区别于 Stage 5.55 的 plan-based 版本）。

### 2.4 §16 接口隔离

输入 `&TraitResolver` + `&Rodeo`，输出 `Vec<String>`。不引用 `mir::ty` /
`Emitter`，无循环依赖。

### 2.5 不修改现有路径

- 所有现有 codegen 函数保持不变
- Stage 5.57 才让 driver 调用这个便捷入口

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1396 + 新增 ~12 = ~1408）
4. §1.2 交付前验收：全绿
5. 输出与 `emit_vtables()` + `emit_dyn_trait_ptrs()` 分别调用的**并集**逐字节一致

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_emit_trait_dispatch_globals_text_batch_from_resolver_empty` | 空 TraitResolver |
| `test_emit_dispatch_globals_text_batch_from_resolver_single` | 单个 vtable |
| `test_emit_dispatch_globals_text_batch_from_resolver_multi` | 多个 vtable |
| `test_match_separate_emit_vtables_and_dyn_trait_ptrs` | **== emit_vtables + emit_dyn_trait_ptrs** |
| `test_match_plan_based_text_batch` | **== plan-based text batch** |
| `test_no_side_effects_on_resolver` | 纯函数 |
| `test_no_emitter_needed` | 无需 Emitter |
| `test_vtable_lines_first` | vtable 行在前 |
| `test_dynptr_lines_second` | dynptr 行在后 |
| `test_count_matches_vtables` | 行数 == 2 × vtables.len() |
| `test_real_scenario` | 模拟真实场景 |
| `test_deterministic` | 重复调用相同结果 |

## 5. 后续依赖

- **Stage 5.57 (codegen trait-dispatch emission refactor)**:
  - driver 调用便捷入口替代分别调用 emit_vtables + emit_dyn_trait_ptrs
  - TextEmitter + emit_*() 委托给 free fn
- **Stage 5.58+ (dyn Trait MIR lowering)**: 直接调用便捷入口

---

**创建日期**: 2026-07-23
