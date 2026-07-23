# Stage 5.57 开发计划：TextEmitter::emit_vtable_global delegation

> **阶段**: Stage 5.57
> **版本**: v0.11.52 → v0.11.53
> **状态**: ✅ Complete

## 1. 目标

**第一个修改现有 codegen 路径的子阶段**——将 `TextEmitter::emit_vtable_global()`
的方法体替换为委托给 Stage 5.44 的 `emit_vtable_global_text()` free function。
这是一行方法体修改，行为等价（14 个交叉验证测试已保证），并顺带修复
null 处理 latent bug（TextEmitter 当前路径不处理 "null" symbol，会 emit
`ptr @null`；委托后将正确 emit `ptr null`）。

## 2. 设计

### 2.1 修改

`src/codegen/text_emitter.rs` 中 `TextEmitter::emit_vtable_global()` 方法体：

**Before** (Stage 5.6 inline):
```rust
fn emit_vtable_global(&mut self, global_name: &str, method_symbols: &[String]) -> EmitValue {
    let init = if method_symbols.is_empty() {
        "zeroinitializer".to_string()
    } else {
        let entries: Vec<String> = method_symbols
            .iter()
            .map(|sym| format!("ptr @{}", sym))
            .collect();
        format!("[{} x ptr] [{}]", method_symbols.len(), entries.join(", "))
    };
    let global_def = format!("@{} = private unnamed_addr constant {}", global_name, init);
    self.globals.push(global_def);
    global_name.to_string()
}
```

**After** (Stage 5.57 delegation):
```rust
fn emit_vtable_global(&mut self, global_name: &str, method_symbols: &[String]) -> EmitValue {
    // Stage 5.57: delegate to emit_vtable_global_text() (Stage 5.44 free function).
    // This also fixes the latent null-handling bug — the old inline code would
    // emit `ptr @null` for "null" strings, while the free function correctly
    // emits `ptr null`.
    let global_def = crate::codegen::emit_vtable_global_text(global_name, method_symbols);
    self.globals.push(global_def);
    global_name.to_string()
}
```

### 2.2 行为等价

- 非 null 路径：**逐字节一致**（Stage 5.44 的交叉验证测试已保证）
- null 路径：**行为改进**——`ptr @null` → `ptr null`（正确处理 missing slots）
- 空路径（zeroinitializer）：**逐字节一致**

### 2.3 命名标准化

无新 API——仅修改现有 trait method 方法体。

### 2.4 §16 接口隔离

`TextEmitter` 调用 `crate::codegen::emit_vtable_global_text()`（同模块 free function），
无跨模块依赖问题。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1408 + 新增 ~10 = ~1418）
4. §1.2 交付前验收：全绿
5. **现有 vtable codegen 测试全部通过**（无回归）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_text_emitter_vtable_global_delegation_basic` | 委托后基本功能正常 |
| `test_text_emitter_vtable_global_delegation_empty` | 空 symbols → zeroinitializer |
| `test_text_emitter_vtable_global_delegation_single` | 单 symbol |
| `test_text_emitter_vtable_global_delegation_multi` | 多 symbols |
| `test_text_emitter_vtable_global_delegation_null` | "null" → ptr null（bug fix） |
| `test_text_emitter_vtable_global_delegation_no_regression` | 现有 vtable codegen 测试无回归 |
| `test_text_emitter_vtable_global_delegation_match_free_fn` | 委托输出 == free fn 输出 |
| `test_text_emitter_vtable_global_delegation_emitter_globals` | globals Vec 正确 |
| `test_text_emitter_vtable_global_delegation_return_value` | 返回 global_name |
| `test_text_emitter_vtable_global_delegation_real_scenario` | 模拟真实场景 |

## 5. 后续依赖

- **Stage 5.58**: `TextEmitter::emit_dyn_trait_const()` 委托给 `emit_dynptr_global_text()`
- **Stage 5.59**: `emit_vtables()` 委托给 `emit_vtables_from_resolver()`
- **Stage 5.60**: `emit_dyn_trait_ptrs()` 委托给 `emit_dynptrs_from_resolver()`

---

**创建日期**: 2026-07-23
