# Stage 5.59 开发计划：emit_vtables delegation

> **阶段**: Stage 5.59
> **版本**: v0.11.54 → v0.11.55
> **状态**: ✅ Complete

## 1. 目标

将 `emit_vtables()` 方法体替换为委托给 Stage 5.47 的
`emit_vtables_from_resolver()` free function。与 Stage 5.57/5.58 模式相同——
一行方法体修改，行为等价（交叉验证测试已保证）。

## 2. 设计

### 2.1 修改

`src/codegen/mod.rs` 中 `emit_vtables()` 函数体：

**Before** (Stage 5.6 inline loop):
```rust
pub fn emit_vtables(trait_resolver: &TraitResolver, interner: &Rodeo, emitter: &mut dyn Emitter) {
    for ((trait_name, self_ty_name), vtable) in &trait_resolver.vtables {
        // ... inline construction + emitter.emit_vtable_global() ...
    }
}
```

**After** (Stage 5.59 delegation):
```rust
pub fn emit_vtables(trait_resolver: &TraitResolver, interner: &Rodeo, emitter: &mut dyn Emitter) {
    // Stage 5.59: delegate to emit_vtables_from_resolver() (Stage 5.47).
    emit_vtables_from_resolver(trait_resolver, interner, emitter)
}
```

### 2.2 行为等价

与旧内联循环**行为完全等价**（Stage 5.47 的两个交叉验证测试已保证）。

### 2.3 §16 接口隔离

`emit_vtables()` 调用同模块 `emit_vtables_from_resolver()`，无跨模块依赖问题。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1428 + 新增 ~8 = ~1436）
4. §1.2 交付前验收：全绿
5. **现有 vtable codegen 测试全部通过**（无回归）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_emit_vtables_delegation_basic` | 委托后基本功能 |
| `test_emit_vtables_delegation_empty` | 空 TraitResolver |
| `test_emit_vtables_delegation_single` | 单个 vtable |
| `test_emit_vtables_delegation_multi` | 多个 vtable |
| `test_emit_vtables_delegation_no_regression` | 现有测试无回归 |
| `test_emit_vtables_delegation_match_orchestrator` | 委托输出 == orchestrator 输出 |
| `test_emit_vtables_delegation_real_scenario` | 模拟真实场景 |
| `test_emit_vtables_delegation_deterministic` | 重复调用相同结果 |

---

**创建日期**: 2026-07-23
