# Stage 5.7 开发计划：dyn Trait fat-pointer 构造

> **阶段**: Stage 5.7
> **版本**: v0.11.5 → v0.11.6
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.19 §17.3 时期 1

## 1. 目标

在 Stage 5.6（vtable codegen 发射）的基础上，构造 `dyn Trait` fat
pointer 的 LLVM IR 全局常量。这是 L5 trait dispatch 的最后一块基础——
有了 fat pointer 全局，后续 Stage 5.8+ 才能在 MIR lowering 中实际生成
`dyn Trait` 值并完成动态分派。

## 2. 背景

Stage 5.6 发射了 `@.vtable.<trait>.<type>` 全局（`[N x ptr]` 数组）。
但 codegen 仍不知道如何构造一个 `dyn Trait` 值——即 `{ ptr (data), ptr (vtable) }`
fat pointer。Stage 5.7 添加这个构造能力。

## 3. 设计

### 3.1 `emit_dyn_trait_ptr_type()`

新增 `pub fn emit_dyn_trait_ptr_type() -> EmitType`，返回
`EmitType::Struct([OpaquePtr, OpaquePtr])`。

与 `emit_fat_ptr_type`（`{ ptr, i64 }` 用于 `&str`/`&[T]`）的区别：
- `&str`/`&[T]` fat pointer = `{ ptr (data), i64 (len) }`
- `dyn Trait` fat pointer = `{ ptr (data), ptr (vtable) }`

两个 `ptr` 都是 opaque，因为具体类型在 `dyn` 边界处被擦除。

### 3.2 `Emitter::emit_dyn_trait_const()`

新增 trait 方法：
```rust
fn emit_dyn_trait_const(
    &mut self,
    global_name: &str,
    data_symbol: &str,
    vtable_symbol: &str,
) -> EmitValue;
```

发射格式：
```llvm
@.dynptr.Foo.S = private unnamed_addr constant
    { ptr, ptr } { ptr @.data.S, ptr @.vtable.Foo.S }
```

### 3.3 `codegen::emit_dyn_trait_ptrs()`

新增 `pub fn emit_dyn_trait_ptrs(trait_resolver, interner, emitter)`，
遍历 `trait_resolver.vtables.keys()`，对每个 (trait, type) pair 调用
`emitter.emit_dyn_trait_const()`。调用点：`codegen_crate` 中
`emit_vtables` 之后。

### 3.4 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `codegen::emit_dyn_trait_ptrs` | `emit_` 前缀 | 与 `emit_vtables` 一致 |
| `codegen::emit_dyn_trait_ptr_type` | `emit_` + `_type` 后缀 | 与 `emit_fat_ptr_type` 一致 |
| `Emitter::emit_dyn_trait_const` | `emit_` 前缀 | 与 `emit_vtable_global` 一致 |

## 4. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 5.7-a | `emit_dyn_trait_ptr_type()` 自由函数 | L1 |
| 5.7-b | `Emitter::emit_dyn_trait_const` trait 方法 | L1 |
| 5.7-c | `TextEmitter::emit_dyn_trait_const` 实现 | L2 |
| 5.7-d | `codegen::emit_dyn_trait_ptrs` 自由函数 | L1 |
| 5.7-e | `codegen_crate` 调用 `emit_dyn_trait_ptrs` | L1 |
| 5.7-f | lib.rs re-export `emit_dyn_trait_ptr_type` + `emit_dyn_trait_ptrs` | L1 |
| 5.7-g | 测试 `dyn_trait_ptr_tests.rs` (4 用例) | L1 |
| 5.7-h | Cargo.toml 版本 + all_tests.rs 模块注册 | L1 |

## 5. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（预期 922 + 4 = 926 passed）
4. §17.3 三阶段文档协议执行
5. §16 合规：codegen 仍是纯 MIR/TraitResolver 消费者
6. API 命名遵循 api-naming-standard §3

---

**创建日期**: 2026-07-22
