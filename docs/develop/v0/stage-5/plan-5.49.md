# Stage 5.49 开发计划：codegen dynptr spec builder

> **阶段**: Stage 5.49
> **版本**: v0.11.44 → v0.11.45
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function `build_dynptr_global_specs()`：
输入 `&TraitResolver` + `&Rodeo`，输出 `Vec<StdlibDynptrGlobalSpec>`——**与
`emit_dyn_trait_ptrs()` 当前内联构造的 spec 列表逐字节一致**。这是把
`emit_dyn_trait_ptrs()` 的"构造 spec"逻辑提取到独立纯函数，为 Stage 5.50
的 `emit_dyn_trait_ptrs()` 重构做准备（届时 `emit_dyn_trait_ptrs()` 调用
`build_dynptr_global_specs()` + 批量 push 到 emitter）。

## 2. 设计

### 2.1 新增类型

```rust
/// 单个 dynptr global 规格：global_name + data_symbol + vtable_symbol。
pub struct StdlibDynptrGlobalSpec {
    pub global_name: String,    // ".dynptr.<trait>.<type>"
    pub data_symbol: String,    // ".data.<type>"
    pub vtable_symbol: String,  // ".vtable.<trait>.<type>"
}
```

### 2.2 新增 API

```rust
/// 从 TraitResolver.vtables 构造 StdlibDynptrGlobalSpec 列表。
/// 与 emit_dyn_trait_ptrs() 当前内联构造逻辑逐字节一致。
pub fn build_dynptr_global_specs(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> Vec<StdlibDynptrGlobalSpec>
```

### 2.3 计算规则（与 emit_dyn_trait_ptrs() 严格一致）

对每个 `(trait_name, self_ty_name)` in `trait_resolver.vtables.keys()`：
- `trait_str = interner.try_resolve(trait_name).unwrap_or("Trait")`
- `type_str = interner.try_resolve(self_ty_name).unwrap_or("Type")`
- `global_name = format!(".dynptr.{}.{}", trait_str, type_str)`
- `data_symbol = format!(".data.{}", type_str)`
- `vtable_symbol = format!(".vtable.{}.{}", trait_str, type_str)`
- push `StdlibDynptrGlobalSpec { global_name, data_symbol, vtable_symbol }`

### 2.4 命名标准化（§23）

| API/类型 | 命名规则 | 合规 |
|----------|---------|------|
| `StdlibDynptrGlobalSpec` | `<Noun><Noun><Noun><Noun>` | ✅ |
| `build_dynptr_global_specs` | `<verb>_<noun>_<adj>_<noun>` | ✅ |
| `global_name` / `data_symbol` / `vtable_symbol` (fields) | `<noun>_<noun>` | ✅ |

命名与 Stage 5.46 `build_vtable_global_specs` / `StdlibVtableGlobalSpec`
对称（vtable → dynptr）。

### 2.5 §16 接口隔离

输入 `&TraitResolver` + `&Rodeo`，输出 `Vec<StdlibDynptrGlobalSpec>`。不引用
`mir::ty` / `Emitter`，无循环依赖。纯函数，可在任意阶段调用。

### 2.6 不修改现有路径

- `emit_dyn_trait_ptrs()` 保持不变
- `TextEmitter::emit_dyn_trait_const()` 保持不变
- Stage 5.50 才让 `emit_dyn_trait_ptrs()` 调用 `build_dynptr_global_specs()` +
  批量 push

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1310 + 新增 ~12 = ~1322）
4. §1.2 交付前验收：全绿
5. 输出与 `emit_dyn_trait_ptrs()` 当前内联构造**逐字节一致**（测试覆盖）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_build_dynptr_global_specs_empty` | 空 TraitResolver → 空 Vec |
| `test_build_dynptr_global_specs_single` | 单个 vtable |
| `test_build_dynptr_global_specs_multi` | 多个 vtable |
| `test_build_dynptr_global_specs_global_name_format` | `.dynptr.<trait>.<type>` 格式 |
| `test_build_dynptr_global_specs_data_symbol` | `.data.<type>` 格式 |
| `test_build_dynptr_global_specs_vtable_symbol` | `.vtable.<trait>.<type>` 格式 |
| `test_build_dynptr_global_specs_unresolved_interner` | interner 未找到 → "Trait"/"Type" 默认 |
| `test_build_dynptr_global_specs_no_side_effects` | 纯函数，不修改输入 |
| `test_build_dynptr_global_specs_deterministic` | 重复调用返回相同结果 |
| `test_build_dynptr_global_specs_match_emit_dyn_trait_ptrs` | **与 emit_dyn_trait_ptrs 内联构造一致** |
| `test_build_dynptr_global_specs_then_emit` | build + emit 验证 |
| `test_build_dynptr_global_specs_real_scenario` | 模拟真实场景 |

## 5. 后续依赖

- **Stage 5.50 (codegen dynptr emission refactor)**:
  - `emit_dyn_trait_ptrs()` 内部调用 `build_dynptr_global_specs()` + 批量 push
  - `TextEmitter::emit_dyn_trait_const()` 委托给 `emit_dynptr_global_text()` (Stage 5.48)
- **Stage 5.51+ (dyn Trait MIR lowering)**: 直接调用 spec builder

---

**创建日期**: 2026-07-23
