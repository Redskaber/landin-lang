# Stage 5.44 开发计划：codegen vtable global text bridge

> **阶段**: Stage 5.44
> **版本**: v0.11.39 → v0.11.40
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function `emit_vtable_global_text()`：
输入 `(global_name: &str, method_symbols: &[String])`——**与 `TextEmitter::emit_vtable_global()`
trait method 完全相同的参数签名**——输出 LLVM IR 文本（String）。

这是 Stage 5.43 的 `emit_vtable_global_from_emission()` 与 Stage 5.45 的
`TextEmitter::emit_vtable_global()` 委托重构之间的**桥接函数**：

```
Stage 5.43: emit_vtable_global_from_emission(&StdlibVtableEmission) -> String
                  ↓ (提取 global_name + method_symbols 字段)
Stage 5.44: emit_vtable_global_text(&str, &[String]) -> String  ← 本轮
                  ↑ (TextEmitter::emit_vtable_global 委托)
Stage 5.45: TextEmitter::emit_vtable_global() 方法体改为调用 free fn
```

这种"先桥接、后委托"策略让每一步都可独立审查：
- 5.43: emission 聚合 → IR 文本（高层 API）
- 5.44: (global_name, symbols) → IR 文本（底层 API，匹配 trait method 签名）
- 5.45: trait method 委托给 5.44 free fn（消除重复逻辑）

## 2. 设计

### 2.1 新增 API

```rust
/// 输入 (global_name, method_symbols)，输出 LLVM IR 文本（一行）。
/// 与 TextEmitter::emit_vtable_global 产生的格式逐字节一致。
/// 是 emit_vtable_global_from_emission() 的底层对应版本。
pub fn emit_vtable_global_text(
    global_name: &str,
    method_symbols: &[String],
) -> String
```

### 2.2 与 Stage 5.43 的关系

`emit_vtable_global_from_emission()` 内部将提取 `emission.global_name` +
`emission.method_symbols`，然后调用 `emit_vtable_global_text()`。这消除
了 5.43 与 TextEmitter 之间的重复 IR 格式化逻辑。

### 2.3 LLVM IR 格式（与 text_emitter.rs:524-552 严格一致）

```
@<global_name> = private unnamed_addr constant [N x ptr] [ptr @sym1, ptr @sym2, ...]
```

边界情况：
- `method_symbols.is_empty()` → `... constant zeroinitializer`
- `method_symbols = ["null", ...]` → `ptr null` 字面量（无 `@` 前缀）

### 2.4 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_vtable_global_text` | `<verb>_<noun>_<adj>_<noun>` | ✅ |

`emit_` 前缀与 codegen 模块其他 free function 一致。`_text` 后缀表明
返回 LLVM IR 文本（String），区别于 trait method 的"副作用"版本。

### 2.5 §16 接口隔离

新函数输入 `&str` + `&[String]`，输出 `String`。不引用 `mir::ty` /
`traits::TraitResolver` / `Emitter` trait / `StdlibVtableEmission`，无
循环依赖。纯函数，可在任意阶段调用。

### 2.6 不修改现有路径

- `emit_vtables()` 保持不变
- `TextEmitter::emit_vtable_global()` 保持不变
- `emit_vtable_global_from_emission()` 保持不变（Stage 5.45 会让它内部
  调用新的 `emit_vtable_global_text()`，但本轮不修改）

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1249 + 新增 ~12 = ~1261）
4. §1.2 交付前验收：全绿
5. 新函数输出与 `TextEmitter::emit_vtable_global()` **逐字节一致**（测试覆盖）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_emit_vtable_global_text_basic` | 基本调用 |
| `test_emit_vtable_global_text_empty` | 空 symbols → zeroinitializer |
| `test_emit_vtable_global_text_single` | 1 symbol |
| `test_emit_vtable_global_text_multi` | 2+ symbols |
| `test_emit_vtable_global_text_null_symbol` | "null" → ptr null |
| `test_emit_vtable_global_text_mixed_null` | 真实符号 + null 混合 |
| `test_emit_vtable_global_text_global_name` | 全局名格式 |
| `test_emit_vtable_global_text_array_type` | [N x ptr] 格式 |
| `test_emit_vtable_global_text_match_text_emitter` | 与 TextEmitter 逐字节一致 |
| `test_emit_vtable_global_text_match_text_emitter_empty` | 空路径交叉验证 |
| `test_emit_vtable_global_text_match_text_emitter_null` | null 路径（TextEmitter 不处理 null，本函数处理） |
| `test_emit_vtable_global_text_no_leading_at_in_input` | 输入 global_name 无 @ 前缀 |

## 5. 后续依赖

- **Stage 5.45 (codegen vtable emission refactor)**: 
  - `emit_vtable_global_from_emission()` 内部调用 `emit_vtable_global_text()`
  - `TextEmitter::emit_vtable_global()` 委托给 `emit_vtable_global_text()`
  - 消除三处重复的 LLVM IR 格式化逻辑
- **Stage 5.46+ (dyn Trait MIR lowering)**: 直接调用 `emit_vtable_global_text()`

---

**创建日期**: 2026-07-23
