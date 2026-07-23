# Stage 5.43 开发计划：codegen vtable emission helper

> **阶段**: Stage 5.43
> **版本**: v0.11.38 → v0.11.39
> **状态**: ✅ Complete

## 1. 目标

在 `src/codegen/mod.rs` 添加新的 free function `emit_vtable_global_from_emission()`：
输入 `&StdlibVtableEmission`，输出 LLVM IR 文本（与 `TextEmitter::emit_vtable_global`
产生的格式**逐字节一致**）。这是 **Stage 5 中第一个修改 codegen 模块**的子阶段
（5.36-5.42 都是纯 stdlib 查询 API），但**不修改现有 emission 路径**——
`emit_vtables()` + `TextEmitter::emit_vtable_global()` 保持不变，新函数是并行
存在的"纯函数版本"，可在测试中直接调用验证 LLVM IR 文本，无需构造 Emitter
trait 对象。

Stage 5.44+ 将把 `emit_vtables()` 内部改为调用 `emit_vtable_global_from_emission()`，
届时 `TextEmitter::emit_vtable_global()` 可委托给这个 free function，消除重复
的 LLVM IR 格式化逻辑。

## 2. 设计

### 2.1 新增 API

```rust
/// 输入 StdlibVtableEmission，输出 LLVM IR 文本（一行）。
/// 与 TextEmitter::emit_vtable_global 产生的格式逐字节一致。
pub fn emit_vtable_global_from_emission(emission: &StdlibVtableEmission) -> String
```

### 2.2 LLVM IR 格式（与 text_emitter.rs:524-552 严格一致）

```
@.vtable.<trait>.<type> = private unnamed_addr constant [N x ptr] [ptr @sym1, ptr @sym2, ...]
```

边界情况：
- `method_symbols.is_empty()` → `... constant zeroinitializer`
- `method_symbols = ["null"]` → `... constant [1 x ptr] [ptr null]`（"null" 字面量）

### 2.3 命名标准化（§23）

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_vtable_global_from_emission` | `<verb>_<noun>_<adj>_<prep>_<noun>` | ✅ |

`emit_` 前缀与 codegen 模块其他 free function 一致
（`emit_vtables` / `emit_dyn_trait_ptrs` / `emit_fat_ptr_type`）。

### 2.4 §16 接口隔离

新函数输入 `&StdlibVtableEmission`（stdlib 内部类型），输出 `String`。
不引用 `mir::ty` / `traits::TraitResolver` / `Emitter` trait，无循环依赖。
纯函数，可在任意阶段调用。

### 2.5 不修改现有路径

- `emit_vtables()` 保持不变（继续遍历 TraitResolver.vtables）
- `TextEmitter::emit_vtable_global()` 保持不变（继续 push 到 self.globals）
- 新函数 `emit_vtable_global_from_emission()` 是**并行存在**的纯函数版本
- Stage 5.44+ 才让 `TextEmitter::emit_vtable_global()` 委托给这个 free function

这种"先并行、后委托"的策略让本轮变更可独立审查，且 LLVM IR 一致性有测试
覆盖。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1236 + 新增 ~12 = ~1248）
4. §1.2 交付前验收：全绿
5. 新函数输出与 `TextEmitter::emit_vtable_global()` **逐字节一致**（测试覆盖）

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_emit_vtable_global_from_emission_clone` | Clone + S + 2 方法 → 完整 IR |
| `test_emit_vtable_global_from_emission_drop` | Drop + S + [drop] → 1 slot IR |
| `test_emit_vtable_global_from_emission_marker` | Copy + S + [] → zeroinitializer |
| `test_emit_vtable_global_from_emission_partial` | Clone + S + [clone] → null slot |
| `test_emit_vtable_global_from_emission_arith` | Add + Vec + [add] → 1 slot |
| `test_emit_vtable_global_from_emission_format_global_name` | 全局名匹配 `.vtable.<trait>.<type>` |
| `test_emit_vtable_global_from_emission_format_array` | 数组类型 `[N x ptr]` 正确 |
| `test_emit_vtable_global_from_emission_format_entries` | `ptr @sym` 格式正确 |
| `test_emit_vtable_global_from_emission_match_text_emitter` | 与 TextEmitter 逐字节一致 |
| `test_emit_vtable_global_from_emission_empty_marker_zeroinitializer` | marker → zeroinitializer |
| `test_emit_vtable_global_from_emission_null_symbol` | "null" → `ptr null` |
| `test_emit_vtable_global_from_emission_partial_eq` | PartialEq + [eq] → [ptr @landin_S_eq, ptr null] |

## 5. 后续依赖

- **Stage 5.44+ (codegen vtable emission refactor)**: 让 `TextEmitter::emit_vtable_global()`
  委托给 `emit_vtable_global_from_emission()`，消除重复的 LLVM IR 格式化逻辑。
- **Stage 5.45+ (dyn Trait MIR lowering)**: MIR lowering 直接调用这个 free function
  生成 vtable 全局文本。

---

**创建日期**: 2026-07-23
