# Stage 5.50 开发计划：codegen dynptr emission orchestrator

> **阶段**: Stage 5.50
> **版本**: v0.11.45 → v0.11.46
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function `emit_dynptrs_from_resolver()`：
**组合** Stage 5.49 的 `build_dynptr_global_specs()` + per-spec
`Emitter::emit_dyn_trait_const()` 调用。这是 `emit_dyn_trait_ptrs()` 当前内联循环的
"纯函数+副作用组合版本"——Stage 5.51 将让 `emit_dyn_trait_ptrs()` 委托给这个
orchestrator。

## 2. 设计

### 2.1 新增 API

```rust
/// 组合 build_dynptr_global_specs + per-spec Emitter::emit_dyn_trait_const。
/// 与 emit_dyn_trait_ptrs() 当前内联循环行为等价。
pub fn emit_dynptrs_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
)
```

### 2.2 计算规则（与 emit_dyn_trait_ptrs() 严格一致）

1. `let specs = build_dynptr_global_specs(trait_resolver, interner);`
2. 对每个 spec 调用 `emitter.emit_dyn_trait_const(&spec.global_name, &spec.data_symbol, &spec.vtable_symbol)`

### 2.3 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_dynptrs_from_resolver` | `<verb>_<noun>_<prep>_<noun>` | ✅ |

命名与 Stage 5.47 `emit_vtables_from_resolver` 对称（vtables → dynptrs）。
`emit_` 前缀表明产生副作用（push 到 emitter）。`_from_resolver` 表明输入来自
TraitResolver。

### 2.4 §16 接口隔离

输入 `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter`（与
`emit_dyn_trait_ptrs()` 完全相同）。不引用 `mir::ty`，无循环依赖。

### 2.5 不修改现有路径

- `emit_dyn_trait_ptrs()` 保持不变
- `TextEmitter::emit_dyn_trait_const()` 保持不变
- `build_dynptr_global_specs()` (Stage 5.49) 保持不变
- Stage 5.51 才让 `emit_dyn_trait_ptrs()` 委托给 `emit_dynptrs_from_resolver()`

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1322 + 新增 ~12 = ~1334）
4. §1.2 交付前验收：全绿
5. 输出与 `emit_dyn_trait_ptrs()` 当前内联循环**行为等价**（测试覆盖）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_emit_dynptrs_from_resolver_empty` | 空 TraitResolver → 不调用 emitter |
| `test_emit_dynptrs_from_resolver_single` | 单个 vtable → 1 次 emitter 调用 |
| `test_emit_dynptrs_from_resolver_multi` | 多个 vtable → 多次 emitter 调用 |
| `test_emit_dynptrs_from_resolver_match_emit_dyn_trait_ptrs` | **与 emit_dyn_trait_ptrs 行为等价** |
| `test_emit_dynptrs_from_resolver_no_side_effects_on_resolver` | 不修改 resolver |
| `test_emit_dynptrs_from_resolver_real_scenario` | 模拟真实场景 |
| `test_emit_dynptrs_from_resolver_unresolved_interner` | interner 未找到 → 默认名 |
| `test_emit_dynptrs_from_resolver_emitter_called_correctly` | emitter 接收正确参数 |
| `test_emit_dynptrs_from_resolver_count_matches_vtables` | 调用次数 == vtables.len() |
| `test_emit_dynptrs_from_resolver_composes_build_and_emit` | 组合 build + emit 验证 |
| `test_emit_dynptrs_from_resolver_deterministic_count` | 重复调用相同次数 |
| `test_emit_dynptrs_from_resolver_match_emit_dyn_trait_ptrs_multi` | 多 vtable 交叉验证 |

## 5. 后续依赖

- **Stage 5.51 (codegen dynptr emission refactor)**:
  - `emit_dyn_trait_ptrs()` 方法体改为 `emit_dynptrs_from_resolver(trait_resolver, interner, emitter)`
  - `TextEmitter::emit_dyn_trait_const()` 委托给 `emit_dynptr_global_text()` (Stage 5.48)
- **Stage 5.52+ (dyn Trait MIR lowering)**: 直接调用 orchestrator

---

**创建日期**: 2026-07-23
