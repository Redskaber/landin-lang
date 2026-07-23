# Stage 5.46 开发计划：codegen vtable spec builder

> **阶段**: Stage 5.46
> **版本**: v0.11.41 → v0.11.42
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function `build_vtable_global_specs()`：
输入 `&TraitResolver` + `&Rodeo`，输出 `Vec<StdlibVtableGlobalSpec>`——**与
`emit_vtables()` 当前内联构造的 spec 列表逐字节一致**。这是把 `emit_vtables()`
的"构造 spec"逻辑提取到独立纯函数，为 Stage 5.47 的 `emit_vtables()` 重构
做准备（届时 `emit_vtables()` 调用 `build_vtable_global_specs()` +
`emit_vtable_globals_batch()` + 批量 push 到 emitter）。

## 2. 设计

### 2.1 新增 API

```rust
/// 从 TraitResolver.vtables 构造 StdlibVtableGlobalSpec 列表。
/// 与 emit_vtables() 当前内联构造逻辑逐字节一致。
pub fn build_vtable_global_specs(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> Vec<StdlibVtableGlobalSpec>
```

### 2.2 计算规则（与 emit_vtables() 严格一致）

对每个 `((trait_name, self_ty_name), vtable)` in `trait_resolver.vtables`：
- `trait_str = interner.try_resolve(trait_name).unwrap_or("Trait")`
- `type_str = interner.try_resolve(self_ty_name).unwrap_or("Type")`
- `global_name = format!(".vtable.{}.{}", trait_str, type_str)`
- `method_symbols = vtable.entries.iter().map(|e| e.fn_name.clone()).collect()`
- push `StdlibVtableGlobalSpec { global_name, method_symbols }`

### 2.3 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `build_vtable_global_specs` | `<verb>_<noun>_<adj>_<noun>` | ✅ |

`build_` 前缀表明这是构造函数（输入数据 → 输出数据），不产生副作用。
`_specs`（复数）表明返回多个 spec。

### 2.4 §16 接口隔离

输入 `&TraitResolver` + `&Rodeo`（与 `emit_vtables()` 相同），输出
`Vec<StdlibVtableGlobalSpec>`。不引用 `mir::ty` / `Emitter`，无循环依赖。
纯函数，可在任意阶段调用。

### 2.5 不修改现有路径

- `emit_vtables()` 保持不变（继续内联构造 + 调用 emitter）
- `TextEmitter::emit_vtable_global()` 保持不变
- `emit_vtable_globals_batch()` (Stage 5.45) 保持不变
- Stage 5.47 才让 `emit_vtables()` 调用 `build_vtable_global_specs()` +
  `emit_vtable_globals_batch()`

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1273 + 新增 ~12 = ~1285）
4. §1.2 交付前验收：全绿
5. 输出与 `emit_vtables()` 当前内联构造**逐字节一致**（测试覆盖）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_build_vtable_global_specs_empty` | 空 TraitResolver → 空 Vec |
| `test_build_vtable_global_specs_single` | 单个 vtable |
| `test_build_vtable_global_specs_multi` | 多个 vtable |
| `test_build_vtable_global_specs_global_name_format` | `.vtable.<trait>.<type>` 格式 |
| `test_build_vtable_global_specs_method_symbols` | method_symbols 从 VtableEntry.fn_name 提取 |
| `test_build_vtable_global_specs_unresolved_interner` | interner 未找到 → "Trait"/"Type" 默认 |
| `test_build_vtable_global_specs_no_side_effects` | 纯函数，不修改输入 |
| `test_build_vtable_global_specs_deterministic` | 重复调用返回相同结果（顺序） |
| `test_build_vtable_global_specs_match_emit_vtables_inline` | **与 emit_vtables 内联构造一致** |
| `test_build_vtable_global_specs_then_batch_emit` | build + batch → 完整 IR 文本 |
| `test_build_vtable_global_specs_empty_vtable_entries` | vtable.entries 空 → 空 method_symbols |
| `test_build_vtable_global_specs_real_scenario` | 模拟真实 TraitResolver 场景 |

## 5. 后续依赖

- **Stage 5.47 (codegen vtable emission refactor)**:
  - `emit_vtables()` 内部调用 `build_vtable_global_specs()` + `emit_vtable_globals_batch()`
  - `TextEmitter::emit_vtable_global()` 委托给 `emit_vtable_global_text()`
- **Stage 5.48+ (dyn Trait MIR lowering)**: 直接调用 spec builder + batch

---

**创建日期**: 2026-07-23
