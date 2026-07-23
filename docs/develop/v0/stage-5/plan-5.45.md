# Stage 5.45 开发计划：codegen vtable emission batch helper

> **阶段**: Stage 5.45
> **版本**: v0.11.40 → v0.11.41
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function `emit_vtable_globals_batch()`：
输入一组 `(global_name, method_symbols)` 规格切片，输出 `Vec<String>`——每
个元素是一行 LLVM IR 文本。这是 Stage 5.44 的 `emit_vtable_global_text()` 的
批量版本，为 Stage 5.46 的 `emit_vtables()` 重构做准备（届时
`emit_vtables()` 可调用 batch helper 一次生成所有 vtable IR 行，再批量
push 到 emitter）。

## 2. 设计

### 2.1 新增类型

```rust
/// 单个 vtable global 规格：global_name + method_symbols。
pub struct StdlibVtableGlobalSpec {
    pub global_name: String,
    pub method_symbols: Vec<String>,
}
```

### 2.2 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `emit_vtable_globals_batch` | `(&[StdlibVtableGlobalSpec]) -> Vec<String>` | 批量生成 vtable IR 文本 |

### 2.3 计算规则

- 对每个 spec 调用 `emit_vtable_global_text(spec.global_name, spec.method_symbols)`
- 收集结果到 `Vec<String>`
- 空 input → 空 Vec

### 2.4 命名标准化（§23）

| API/类型 | 命名规则 | 合规 |
|----------|---------|------|
| `StdlibVtableGlobalSpec` | `<Noun><Noun><Noun><Noun>` | ✅ |
| `emit_vtable_globals_batch` | `<verb>_<noun>_<adj>_<noun>` | ✅ |
| `global_name` / `method_symbols` (fields) | `<noun>_<noun>` | ✅ |

`emit_` 前缀一致；`_batch` 后缀表明批量版本。`_globals`（复数）区别于
Stage 5.44 的 `emit_vtable_global_text`（单数）。

### 2.5 §16 接口隔离

`StdlibVtableGlobalSpec` 仅依赖 `String` + `Vec<String>`，不引用
`mir::ty` / `traits::TraitResolver` / `Emitter` / `StdlibVtableEmission`，
无循环依赖。纯函数，可在任意阶段调用。

### 2.6 不修改现有路径

- `emit_vtables()` 保持不变
- `TextEmitter::emit_vtable_global()` 保持不变
- `emit_vtable_global_text()` (Stage 5.44) 保持不变
- `emit_vtable_global_from_emission()` (Stage 5.43) 保持不变

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1261 + 新增 ~12 = ~1273）
4. §1.2 交付前验收：全绿
5. 批量输出与逐个调用 `emit_vtable_global_text()` **逐字节一致**（测试覆盖）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_emit_vtable_globals_batch_empty` | 空 input → 空 Vec |
| `test_emit_vtable_globals_batch_single` | 单个 spec |
| `test_emit_vtable_globals_batch_multi` | 多个 spec |
| `test_emit_vtable_globals_batch_matches_individual` | 批量 == 逐个调用 |
| `test_emit_vtable_globals_batch_order_preserved` | 顺序保留 |
| `test_emit_vtable_globals_batch_with_marker` | 含 marker (zeroinitializer) |
| `test_emit_vtable_globals_batch_with_null` | 含 null symbol |
| `test_emit_vtable_globals_batch_mixed` | 混合 marker + null + real |
| `test_stdlib_vtable_global_spec_struct` | struct 字段访问 |
| `test_stdlib_vtable_global_spec_eq` | PartialEq/Eq 派生 |
| `test_emit_vtable_globals_batch_real_vtables` | 模拟真实 emit_vtables 场景 |
| `test_emit_vtable_globals_batch_dedup_not_required` | 不去重（调用方负责） |

## 5. 后续依赖

- **Stage 5.46 (codegen vtable emission refactor)**:
  - `emit_vtables()` 内部构造 `Vec<StdlibVtableGlobalSpec>`，调用
    `emit_vtable_globals_batch()`，再批量 push 到 emitter
  - `TextEmitter::emit_vtable_global()` 委托给 `emit_vtable_global_text()`
- **Stage 5.47+ (dyn Trait MIR lowering)**: 直接调用 batch helper

---

**创建日期**: 2026-07-23
