# Stage 5.51 开发计划：codegen vtable + dynptr combined emission orchestrator

> **阶段**: Stage 5.51
> **版本**: v0.11.46 → v0.11.47
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function
`emit_vtables_and_dynptrs_from_resolver()`：**组合** Stage 5.47 的
`emit_vtables_from_resolver()` + Stage 5.50 的
`emit_dynptrs_from_resolver()`。这是 codegen 发射所有 trait-dispatch globals
（vtable + dynptr）的**单一入口点**，为 Stage 5.52 的 codegen 重构做准备
（届时 driver 可调用这一个函数替代分别调用 `emit_vtables()` + `emit_dyn_trait_ptrs()`）。

## 2. 设计

### 2.1 新增 API

```rust
/// 组合 emit_vtables_from_resolver + emit_dynptrs_from_resolver。
/// 一次调用发射所有 trait-dispatch globals (vtable + dynptr)。
pub fn emit_vtables_and_dynptrs_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
)
```

### 2.2 计算规则

1. 调用 `emit_vtables_from_resolver(trait_resolver, interner, emitter)` —— 发射所有 vtable globals
2. 调用 `emit_dynptrs_from_resolver(trait_resolver, interner, emitter)` —— 发射所有 dynptr globals

### 2.3 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_vtables_and_dynptrs_from_resolver` | `<verb>_<noun>_<conj>_<noun>_<prep>_<noun>` | ✅ |

`emit_` 前缀一致（产生副作用——push 到 emitter）。`_and_` 连接两个名词
（vtables + dynptrs），`_from_resolver` 表明输入来自 TraitResolver。

### 2.4 §16 接口隔离

输入 `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter`。不引用 `mir::ty`，无
循环依赖。

### 2.5 不修改现有路径

- `emit_vtables()` 保持不变
- `emit_dyn_trait_ptrs()` 保持不变
- `emit_vtables_from_resolver()` (Stage 5.47) 保持不变
- `emit_dynptrs_from_resolver()` (Stage 5.50) 保持不变
- Stage 5.52 才让 driver/codegen 调用这个 combined orchestrator

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1334 + 新增 ~12 = ~1346）
4. §1.2 交付前验收：全绿
5. 输出 == `emit_vtables()` + `emit_dyn_trait_ptrs()` 分别调用的**并集**（测试覆盖）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_emit_vtables_and_dynptrs_empty` | 空 TraitResolver → 不调用 emitter |
| `test_emit_vtables_and_dynptrs_single` | 单个 vtable → vtable + dynptr global |
| `test_emit_vtables_and_dynptrs_multi` | 多个 vtable → 多 vtable + 多 dynptr |
| `test_emit_vtables_and_dynptrs_match_separate_calls` | **== emit_vtables + emit_dyn_trait_ptrs** |
| `test_emit_vtables_and_dynptrs_no_side_effects_on_resolver` | 不修改 resolver |
| `test_emit_vtables_and_dynptrs_real_scenario` | 模拟真实场景 |
| `test_emit_vtables_and_dynptrs_unresolved_interner` | interner 未找到 → 默认名 |
| `test_emit_vtables_and_dynptrs_emitter_called_correctly` | emitter 接收正确参数 |
| `test_emit_vtables_and_dynptrs_count_matches_vtables` | vtable + dynptr 数 == 2 × vtables.len() |
| `test_emit_vtables_and_dynptrs_composes_both` | 组合两者验证 |
| `test_emit_vtables_and_dynptrs_deterministic_count` | 重复调用相同次数 |
| `test_emit_vtables_and_dynptrs_order` | vtable 在 dynptr 前 |

## 5. 后续依赖

- **Stage 5.52 (codegen trait-dispatch emission refactor)**:
  - driver/codegen 调用 `emit_vtables_and_dynptrs_from_resolver()` 替代分别调用
  - `TextEmitter::emit_vtable_global()` / `emit_dyn_trait_const()` 委托给 free fn
- **Stage 5.53+ (dyn Trait MIR lowering)**: 直接调用 combined orchestrator

---

**创建日期**: 2026-07-23
