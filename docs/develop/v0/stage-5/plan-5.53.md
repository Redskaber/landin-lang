# Stage 5.53 开发计划：codegen trait-dispatch emission plan

> **阶段**: Stage 5.53
> **版本**: v0.11.48 → v0.11.49
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function
`build_trait_dispatch_emission_plan()`：输入 `&TraitResolver` + `&Rodeo`，
输出 `CodegenTraitDispatchEmissionPlan`——**一次调用返回 codegen 发射所有
trait-dispatch globals 所需的全部信息**（vtable specs + dynptr specs + summary）。
这是 Stage 5.46 `build_vtable_global_specs()` + Stage 5.49
`build_dynptr_global_specs()` + Stage 5.52
`build_trait_dispatch_emission_summary()` 的**最终聚合 API**。

## 2. 设计

### 2.1 新增类型

```rust
/// codegen 发射所有 trait-dispatch globals 所需的全部信息。
pub struct CodegenTraitDispatchEmissionPlan {
    pub vtable_specs: Vec<StdlibVtableGlobalSpec>,
    pub dynptr_specs: Vec<StdlibDynptrGlobalSpec>,
    pub summary: CodegenTraitDispatchEmissionSummary,
}
```

### 2.2 新增 API

```rust
/// 一次调用返回 vtable specs + dynptr specs + summary。
pub fn build_trait_dispatch_emission_plan(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> CodegenTraitDispatchEmissionPlan
```

### 2.3 计算规则

- `vtable_specs` = `build_vtable_global_specs(trait_resolver, interner)` (Stage 5.46)
- `dynptr_specs` = `build_dynptr_global_specs(trait_resolver, interner)` (Stage 5.49)
- `summary` = `build_trait_dispatch_emission_summary(trait_resolver, interner)` (Stage 5.52)

### 2.4 命名标准化（§23）

| API/类型 | 命名规则 | 合规 |
|----------|---------|------|
| `CodegenTraitDispatchEmissionPlan` | `<Noun><Noun><Noun><Noun><Noun>` | ✅ |
| `build_trait_dispatch_emission_plan` | `<verb>_<noun>_<noun>_<noun>_<noun>` | ✅ |
| `vtable_specs` / `dynptr_specs` / `summary` (fields) | `<noun>_<noun>` / `<noun>` | ✅ |

### 2.5 §16 接口隔离

输入 `&TraitResolver` + `&Rodeo`，输出 `CodegenTraitDispatchEmissionPlan`。
不引用 `mir::ty` / `Emitter`，无循环依赖。纯函数，可在任意阶段调用。

### 2.6 不修改现有路径

- 所有现有 codegen 函数保持不变
- Stage 5.54 才让 driver/codegen 调用这个 plan

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1360 + 新增 ~12 = ~1372）
4. §1.2 交付前验收：全绿

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_build_trait_dispatch_emission_plan_empty` | 空 TraitResolver |
| `test_build_trait_dispatch_emission_plan_single` | 单个 vtable |
| `test_build_trait_dispatch_emission_plan_multi` | 多个 vtable |
| `test_build_trait_dispatch_emission_plan_vtable_specs` | vtable_specs 正确 |
| `test_build_trait_dispatch_emission_plan_dynptr_specs` | dynptr_specs 正确 |
| `test_build_trait_dispatch_emission_plan_summary` | summary 正确 |
| `test_build_trait_dispatch_emission_plan_match_separate_calls` | == 三个分别调用 |
| `test_build_trait_dispatch_emission_plan_no_side_effects` | 纯函数 |
| `test_build_trait_dispatch_emission_plan_real_scenario` | 模拟真实场景 |
| `test_build_trait_dispatch_emission_plan_unresolved_interner` | interner 未找到 |
| `test_build_trait_dispatch_emission_plan_struct_eq` | PartialEq/Eq 派生 |
| `test_build_trait_dispatch_emission_plan_field_access` | 字段访问 |

## 5. 后续依赖

- **Stage 5.54 (codegen trait-dispatch emission refactor)**:
  - driver 调用 plan，再用 plan.vtable_specs + plan.dynptr_specs 发射
  - TextEmitter + emit_*() 委托给 free fn
- **Stage 5.55+ (dyn Trait MIR lowering)**: 直接调用 plan

---

**创建日期**: 2026-07-23
