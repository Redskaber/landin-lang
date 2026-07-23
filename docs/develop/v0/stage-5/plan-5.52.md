# Stage 5.52 开发计划：codegen trait-dispatch emission summary

> **阶段**: Stage 5.52
> **版本**: v0.11.47 → v0.11.48
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function
`build_trait_dispatch_emission_summary()`：输入 `&TraitResolver` + `&Rodeo`，
输出 `CodegenTraitDispatchEmissionSummary`——项目级 trait-dispatch emission
统计（vtable 数 + dynptr 数 + 总全局数 + 涉及的 trait 名 + 涉及的 type 名）。
这是 Stage 5.42 `stdlib_vtable_emission_summary()` 的 codegen 对应版本，但
从 TraitResolver 计算，为 codegen 诊断输出做准备。

## 2. 设计

### 2.1 新增类型

```rust
/// 项目级 trait-dispatch emission 统计摘要。
pub struct CodegenTraitDispatchEmissionSummary {
    pub vtable_count: u32,          // vtable globals 数
    pub dynptr_count: u32,          // dynptr globals 数
    pub total_global_count: u32,    // vtable + dynptr 总数
    pub trait_names: Vec<String>,   // 涉及的 trait 名（去重）
    pub type_names: Vec<String>,    // 涉及的 type 名（去重）
    pub total_method_slots: u32,    // 所有 vtable 的 slot 总数
}
```

### 2.2 新增 API

```rust
/// 从 TraitResolver 构造 trait-dispatch emission 统计摘要。
pub fn build_trait_dispatch_emission_summary(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> CodegenTraitDispatchEmissionSummary
```

### 2.3 计算规则

- `vtable_count` = `trait_resolver.vtables.len()`
- `dynptr_count` = `trait_resolver.vtables.len()`（每个 (trait, type) 对一个 dynptr）
- `total_global_count` = `vtable_count + dynptr_count`
- `trait_names` = 所有 vtables keys 的 trait_str 去重
- `type_names` = 所有 vtables keys 的 type_str 去重
- `total_method_slots` = 所有 vtable.entries.len() 之和

### 2.4 命名标准化（§23）

| API/类型 | 命名规则 | 合规 |
|----------|---------|------|
| `CodegenTraitDispatchEmissionSummary` | `<Noun><Noun><Noun><Noun><Noun>` | ✅ |
| `build_trait_dispatch_emission_summary` | `<verb>_<noun>_<noun>_<noun>_<noun>` | ✅ |
| `vtable_count` / `dynptr_count` / `total_global_count` / `trait_names` / `type_names` / `total_method_slots` | fields | ✅ |

### 2.5 §16 接口隔离

输入 `&TraitResolver` + `&Rodeo`，输出 `CodegenTraitDispatchEmissionSummary`。
不引用 `mir::ty` / `Emitter`，无循环依赖。纯函数，可在任意阶段调用。

### 2.6 不修改现有路径

- 所有现有 codegen 函数保持不变
- Stage 5.53 才让 driver/codegen 调用这个 summary 做诊断输出

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1346 + 新增 ~12 = ~1358）
4. §1.2 交付前验收：全绿

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_build_trait_dispatch_emission_summary_empty` | 空 TraitResolver → 全 0 |
| `test_build_trait_dispatch_emission_summary_single` | 单个 vtable |
| `test_build_trait_dispatch_emission_summary_multi` | 多个 vtable |
| `test_build_trait_dispatch_emission_summary_vtable_count` | vtable 计数 |
| `test_build_trait_dispatch_emission_summary_dynptr_count` | dynptr 计数 |
| `test_build_trait_dispatch_emission_summary_total_global_count` | 总全局数 |
| `test_build_trait_dispatch_emission_summary_trait_names_dedup` | trait 名去重 |
| `test_build_trait_dispatch_emission_summary_type_names_dedup` | type 名去重 |
| `test_build_trait_dispatch_emission_summary_total_method_slots` | slot 总数 |
| `test_build_trait_dispatch_emission_summary_unresolved_interner` | interner 未找到 → 默认名 |
| `test_build_trait_dispatch_emission_summary_no_side_effects` | 纯函数 |
| `test_build_trait_dispatch_emission_summary_real_scenario` | 模拟真实场景 |

## 5. 后续依赖

- **Stage 5.53 (codegen trait-dispatch emission refactor)**:
  - driver 调用 summary 做诊断输出
  - TextEmitter + emit_*() 委托给 free fn
- **Stage 5.54+ (dyn Trait MIR lowering)**: 直接调用 summary

---

**创建日期**: 2026-07-23
