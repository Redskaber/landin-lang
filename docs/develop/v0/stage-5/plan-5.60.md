# Stage 5.60 开发计划：emit_dyn_trait_ptrs delegation

> **阶段**: Stage 5.60
> **版本**: v0.11.55 → v0.11.56
> **状态**: ✅ Complete

## 1. 目标

**第四个也是最后一个修改现有 codegen 路径的子阶段**——将
`emit_dyn_trait_ptrs()` 函数体替换为委托给 Stage 5.50 的
`emit_dynptrs_from_resolver()` free function。完成后 codegen 的
trait-dispatch emission 逻辑将**完全**集中在 free function。

## 2. 设计

### 2.1 修改

`src/codegen/mod.rs` 中 `emit_dyn_trait_ptrs()` 函数体：

**Before** (Stage 5.7 inline loop):
```rust
pub fn emit_dyn_trait_ptrs(trait_resolver: &TraitResolver, interner: &Rodeo, emitter: &mut dyn Emitter) {
    for (trait_name, self_ty_name) in trait_resolver.vtables.keys() {
        // ... inline construction + emitter.emit_dyn_trait_const() ...
    }
}
```

**After** (Stage 5.60 delegation):
```rust
pub fn emit_dyn_trait_ptrs(trait_resolver: &TraitResolver, interner: &Rodeo, emitter: &mut dyn Emitter) {
    // Stage 5.60: delegate to emit_dynptrs_from_resolver() (Stage 5.50).
    emit_dynptrs_from_resolver(trait_resolver, interner, emitter)
}
```

### 2.2 行为等价

与旧内联循环**行为完全等价**（Stage 5.50 的两个交叉验证测试已保证）。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1435 + 新增 ~7 = ~1442）
4. §1.2 交付前验收：全绿
5. **现有 dynptr codegen 测试全部通过**（无回归）

---

**创建日期**: 2026-07-23
