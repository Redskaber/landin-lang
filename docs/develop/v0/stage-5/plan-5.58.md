# Stage 5.58 开发计划：TextEmitter::emit_dyn_trait_const delegation

> **阶段**: Stage 5.58
> **版本**: v0.11.53 → v0.11.54
> **状态**: ✅ Complete

## 1. 目标

将 `TextEmitter::emit_dyn_trait_const()` 方法体替换为委托给 Stage 5.48 的
`emit_dynptr_global_text()` free function。与 Stage 5.57 模式相同——
一行方法体修改，行为等价（交叉验证测试已保证），消除重复的 LLVM IR
格式化逻辑。

## 2. 设计

### 2.1 修改

`src/codegen/text_emitter.rs` 中 `TextEmitter::emit_dyn_trait_const()` 方法体：

**Before** (Stage 5.7 inline):
```rust
fn emit_dyn_trait_const(&mut self, global_name: &str, data_symbol: &str, vtable_symbol: &str) -> EmitValue {
    let init = format!("{{ ptr, ptr }} {{ ptr @{}, ptr @{} }}", data_symbol, vtable_symbol);
    let global_def = format!("@{} = private unnamed_addr constant {}", global_name, init);
    self.globals.push(global_def);
    global_name.to_string()
}
```

**After** (Stage 5.58 delegation):
```rust
fn emit_dyn_trait_const(&mut self, global_name: &str, data_symbol: &str, vtable_symbol: &str) -> EmitValue {
    // Stage 5.58: delegate to emit_dynptr_global_text() (Stage 5.48 free function).
    let global_def = crate::codegen::emit_dynptr_global_text(global_name, data_symbol, vtable_symbol);
    self.globals.push(global_def);
    global_name.to_string()
}
```

### 2.2 行为等价

所有路径**逐字节一致**（Stage 5.48 的交叉验证测试已保证）。
dynptr 无 null 处理问题（与 vtable 不同），所以无 bug 修复。

### 2.3 §16 接口隔离

`TextEmitter` 调用 `crate::codegen::emit_dynptr_global_text()`（同模块 free function），
无跨模块依赖问题。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1418 + 新增 ~10 = ~1428）
4. §1.2 交付前验收：全绿
5. **现有 dynptr codegen 测试全部通过**（无回归）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_text_emitter_dynptr_delegation_basic` | 委托后基本功能 |
| `test_text_emitter_dynptr_delegation_format` | 格式验证 |
| `test_text_emitter_dynptr_delegation_foo_s` | Foo + S 例子 |
| `test_text_emitter_dynptr_delegation_no_regression` | emit_dyn_trait_ptrs 无回归 |
| `test_text_emitter_dynptr_delegation_match_free_fn` | 委托输出 == free fn 输出 |
| `test_text_emitter_dynptr_delegation_emitter_globals` | globals Vec 正确 |
| `test_text_emitter_dynptr_delegation_return_value` | 返回 global_name |
| `test_text_emitter_dynptr_delegation_data_vtable_symbols` | data + vtable symbol 正确 |
| `test_text_emitter_dynptr_delegation_real_scenario` | 模拟真实场景 |
| `test_text_emitter_dynptr_delegation_multiple` | 多个 dynptr globals |

---

**创建日期**: 2026-07-23
