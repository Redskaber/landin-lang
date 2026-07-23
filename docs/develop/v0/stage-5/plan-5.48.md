# Stage 5.48 开发计划：codegen dynptr global text helper

> **阶段**: Stage 5.48
> **版本**: v0.11.43 → v0.11.44
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function `emit_dynptr_global_text()`：
输入 `(global_name, data_symbol, vtable_symbol)`——**与
`TextEmitter::emit_dyn_trait_const()` trait method 完全相同的参数签名**——
输出 LLVM IR 文本（String）。这是 Stage 5.44 的 `emit_vtable_global_text()` 的
**dynptr 对应版本**，为 Stage 5.49 的 `TextEmitter::emit_dyn_trait_const()`
委托重构做准备。

## 2. 设计

### 2.1 新增 API

```rust
/// 输入 (global_name, data_symbol, vtable_symbol)，输出 LLVM IR 文本。
/// 与 TextEmitter::emit_dyn_trait_const 产生的格式逐字节一致。
pub fn emit_dynptr_global_text(
    global_name: &str,
    data_symbol: &str,
    vtable_symbol: &str,
) -> String
```

### 2.2 LLVM IR 格式（与 text_emitter.rs:554-577 严格一致）

```
@<global_name> = private unnamed_addr constant { ptr, ptr } { ptr @<data_symbol>, ptr @<vtable_symbol> }
```

例：`@.dynptr.Foo.S = private unnamed_addr constant { ptr, ptr } { ptr @.data.S, ptr @.vtable.Foo.S }`

### 2.3 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_dynptr_global_text` | `<verb>_<noun>_<adj>_<noun>` | ✅ |

`emit_` 前缀一致。`_text` 后缀表明返回 LLVM IR 文本（String），区别于
trait method 的"副作用"版本。命名与 Stage 5.44 `emit_vtable_global_text`
对称（vtable → dynptr）。

### 2.4 §16 接口隔离

新函数输入 `&str` × 3，输出 `String`。不引用 `mir::ty` /
`traits::TraitResolver` / `Emitter` trait / `StdlibVtableEmission`，无循环依赖。

### 2.5 不修改现有路径

- `emit_dyn_trait_ptrs()` 保持不变
- `TextEmitter::emit_dyn_trait_const()` 保持不变
- Stage 5.49 才让 `TextEmitter::emit_dyn_trait_const()` 委托给这个 free fn

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1298 + 新增 ~12 = ~1310）
4. §1.2 交付前验收：全绿
5. 新函数输出与 `TextEmitter::emit_dyn_trait_const()` **逐字节一致**（测试覆盖）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_emit_dynptr_global_text_basic` | 基本调用 |
| `test_emit_dynptr_global_text_format` | 格式验证 |
| `test_emit_dynptr_global_text_global_name` | 全局名 |
| `test_emit_dynptr_global_text_data_symbol` | data symbol |
| `test_emit_dynptr_global_text_vtable_symbol` | vtable symbol |
| `test_emit_dynptr_global_text_no_leading_at_in_input` | 输入无 @ 前缀 |
| `test_emit_dynptr_global_text_struct_type` | { ptr, ptr } 类型 |
| `test_emit_dynptr_global_text_match_text_emitter` | **与 TextEmitter 逐字节一致** |
| `test_emit_dynptr_global_text_real_scenario` | 模拟真实场景 |
| `test_emit_dynptr_global_text_foo_s` | Foo + S 例子 |
| `test_emit_dynptr_global_text_display_vec` | Display + Vec 例子 |
| `test_emit_dynptr_global_text_constants` | 多个常量值 |

## 5. 后续依赖

- **Stage 5.49 (codegen dynptr emission refactor)**:
  - `TextEmitter::emit_dyn_trait_const()` 委托给 `emit_dynptr_global_text()`
- **Stage 5.50+ (dyn Trait MIR lowering)**: 直接调用 free fn

---

**创建日期**: 2026-07-23
