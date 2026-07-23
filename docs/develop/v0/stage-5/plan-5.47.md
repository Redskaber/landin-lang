# Stage 5.47 开发计划：codegen vtable emission orchestrator

> **阶段**: Stage 5.47
> **版本**: v0.11.42 → v0.11.43
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function `emit_vtables_from_resolver()`：
**组合** Stage 5.46 的 `build_vtable_global_specs()` + Stage 5.45 的
`emit_vtable_globals_batch()` + 通过 `Emitter::emit_vtable_global()` 批量
push 到 emitter。这是 `emit_vtables()` 当前内联循环的**纯函数+副作用组合版本**——
Stage 5.48 将让 `emit_vtables()` 委托给这个 orchestrator。

## 2. 设计

### 2.1 新增 API

```rust
/// 组合 build_vtable_global_specs + emit_vtable_globals_batch + 批量 push。
/// 与 emit_vtables() 当前内联循环行为等价。
pub fn emit_vtables_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
)
```

### 2.2 计算规则（与 emit_vtables() 严格一致）

1. `let specs = build_vtable_global_specs(trait_resolver, interner);`
2. 对每个 spec 调用 `emitter.emit_vtable_global(&spec.global_name, &spec.method_symbols)`

注意：本轮**不**使用 `emit_vtable_globals_batch()`——因为 `Emitter` trait 当前
的 `emit_vtable_global()` 接收 `(global_name, method_symbols)`，而非预格式化的
IR 文本。Stage 5.48 委托 `TextEmitter::emit_vtable_global()` 给
`emit_vtable_global_text()` 后，才能直接用 batch 生成的 IR 文本批量 push。

### 2.3 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_vtables_from_resolver` | `<verb>_<noun>_<prep>_<noun>` | ✅ |

`emit_` 前缀一致（产生副作用——push 到 emitter）。`_from_resolver` 表明
输入来自 TraitResolver。

### 2.4 §16 接口隔离

输入 `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter`（与 `emit_vtables()`
完全相同）。不引用 `mir::ty`，无循环依赖。

### 2.5 不修改现有路径

- `emit_vtables()` 保持不变
- `TextEmitter::emit_vtable_global()` 保持不变
- `build_vtable_global_specs()` (Stage 5.46) 保持不变
- Stage 5.48 才让 `emit_vtables()` 委托给 `emit_vtables_from_resolver()`

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1285 + 新增 ~12 = ~1297）
4. §1.2 交付前验收：全绿
5. 输出与 `emit_vtables()` 当前内联循环**行为等价**（测试覆盖）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_emit_vtables_from_resolver_empty` | 空 TraitResolver → 不调用 emitter |
| `test_emit_vtables_from_resolver_single` | 单个 vtable → 1 次 emitter 调用 |
| `test_emit_vtables_from_resolver_multi` | 多个 vtable → 多次 emitter 调用 |
| `test_emit_vtables_from_resolver_match_emit_vtables` | **与 emit_vtables 行为等价** |
| `test_emit_vtables_from_resolver_no_side_effects_on_resolver` | 不修改 resolver |
| `test_emit_vtables_from_resolver_real_scenario` | 模拟真实场景（S impls Clone+Drop+Display） |
| `test_emit_vtables_from_resolver_empty_entries` | vtable.entries 空 → 仍调用 emitter |
| `test_emit_vtables_from_resolver_unresolved_interner` | interner 未找到 → 默认名 |
| `test_emit_vtables_from_resolver_emitter_called_correctly` | emitter 接收正确参数 |
| `test_emit_vtables_from_resolver_count_matches_vtables` | 调用次数 == vtables.len() |
| `test_emit_vtables_from_resolver_composes_build_and_emit` | 组合 build + emit 验证 |
| `test_emit_vtables_from_resolver_deterministic_count` | 重复调用相同次数 |

## 5. 后续依赖

- **Stage 5.48 (codegen vtable emission refactor)**:
  - `emit_vtables()` 方法体改为 `emit_vtables_from_resolver(trait_resolver, interner, emitter)`
  - `TextEmitter::emit_vtable_global()` 委托给 `emit_vtable_global_text()`
- **Stage 5.49+ (dyn Trait MIR lowering)**: 直接调用 orchestrator

---

**创建日期**: 2026-07-23
