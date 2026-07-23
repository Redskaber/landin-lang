# Stage 5.54 开发计划：codegen trait-dispatch emission orchestrator (plan-based)

> **阶段**: Stage 5.54
> **版本**: v0.11.49 → v0.11.50
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function
`emit_trait_dispatch_globals_from_plan()`：输入 `&CodegenTraitDispatchEmissionPlan` +
`&mut dyn Emitter`，通过遍历 plan 的 vtable_specs + dynptr_specs 发射所有
trait-dispatch globals。这是**第一个 plan-based orchestrator**——Stage 5.55
driver 重构将调用 `build_trait_dispatch_emission_plan()` + 这个 orchestrator，
替代分别调用 `emit_vtables()` + `emit_dyn_trait_ptrs()`。

## 2. 设计

### 2.1 新增 API

```rust
/// 从 plan 发射所有 trait-dispatch globals (vtable + dynptr)。
pub fn emit_trait_dispatch_globals_from_plan(
    plan: &CodegenTraitDispatchEmissionPlan,
    emitter: &mut dyn Emitter,
)
```

### 2.2 计算规则

1. 对每个 `plan.vtable_specs` 中的 spec，调用 `emitter.emit_vtable_global(&spec.global_name, &spec.method_symbols)`
2. 对每个 `plan.dynptr_specs` 中的 spec，调用 `emitter.emit_dyn_trait_const(&spec.global_name, &spec.data_symbol, &spec.vtable_symbol)`

### 2.3 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_trait_dispatch_globals_from_plan` | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` | ✅ |

`emit_` 前缀一致（产生副作用）。`_from_plan` 表明输入来自 plan（区别于
Stage 5.51 的 `_from_resolver`）。

### 2.4 §16 接口隔离

输入 `&CodegenTraitDispatchEmissionPlan` + `&mut dyn Emitter`。不引用
`mir::ty` / `TraitResolver` / `Rodeo`，无循环依赖。

### 2.5 不修改现有路径

- 所有现有 codegen 函数保持不变
- Stage 5.55 才让 driver 调用 plan + 这个 orchestrator

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1372 + 新增 ~12 = ~1384）
4. §1.2 交付前验收：全绿
5. 输出与 `emit_vtables_and_dynptrs_from_resolver()` **行为等价**（测试覆盖）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_emit_trait_dispatch_globals_from_plan_empty` | 空 plan → 不调用 emitter |
| `test_emit_trait_dispatch_globals_from_plan_single` | 单 spec |
| `test_emit_trait_dispatch_globals_from_plan_multi` | 多 spec |
| `test_emit_trait_dispatch_globals_from_plan_match_resolver_orchestrator` | **== emit_vtables_and_dynptrs_from_resolver** |
| `test_emit_trait_dispatch_globals_from_plan_no_side_effects_on_plan` | 不修改 plan |
| `test_emit_trait_dispatch_globals_from_plan_vtable_emitted` | vtable global 发射 |
| `test_emit_trait_dispatch_globals_from_plan_dynptr_emitted` | dynptr global 发射 |
| `test_emit_trait_dispatch_globals_from_plan_count_matches` | vtable + dynptr 数 == 2 × specs |
| `test_emit_trait_dispatch_globals_from_plan_order` | vtable 在 dynptr 前 |
| `test_emit_trait_dispatch_globals_from_plan_real_scenario` | 模拟真实场景 |
| `test_emit_trait_dispatch_globals_from_plan_composes_plan_and_emit` | 组合 plan + emit |
| `test_emit_trait_dispatch_globals_from_plan_deterministic_count` | 重复调用相同次数 |

## 5. 后续依赖

- **Stage 5.55 (codegen trait-dispatch emission refactor)**:
  - driver 调用 `build_trait_dispatch_emission_plan()` + `emit_trait_dispatch_globals_from_plan()`
  - TextEmitter + emit_*() 委托给 free fn
- **Stage 5.56+ (dyn Trait MIR lowering)**: 直接调用 plan + orchestrator

---

**创建日期**: 2026-07-23
