# Stage 5.6 开发计划：vtable codegen 发射

> **阶段**: Stage 5.6
> **版本**: v0.11.4 → v0.11.5
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.19 §17.3 时期 1

## 1. 目标

将 TraitResolver 收集的 vtable 数据发射为 LLVM IR 全局常量。这是 L5
trait dispatch 的最后一块基础——有了 vtable 全局，后续 Stage 5.7+ 才能
构造 `dyn Trait` fat pointer 并完成动态分派。

## 2. 背景

Stage 5.5 添加了 `VtableEntry` + `Vtable` 数据结构，并在 `collect()` 时
为每个 `impl Trait for Type` 构建 vtable。但 vtable 当时只存在于
TraitResolver 内存中，codegen 不知道它们的存在。

Stage 5.5 同时存在一个隐式缺陷：`VtableEntry.fn_def_id` 实际指向 impl
块的 DefId，而不是 impl 方法的 DefId（HIR 不为 impl 方法分配独立 DefId）。
Stage 5.6 修复这个缺陷 + 完成 codegen 发射。

## 3. 设计

### 3.1 VtableEntry 改造

将 `fn_def_id: DefId` 替换为 `fn_name: String`——直接存储解析后的 LLVM
符号名（`landin_<Type>_<method>`）。理由（§15 最优 > 最小）：
1. TraitResolver 在 `collect()` 时已持有 `&Rodeo`，可一次性解析符号
2. codegen 直接读 `fn_name` 字符串，零跨阶段查询
3. 避免 driver 与 TraitResolver 在命名规则上漂移

### 3.2 driver body_metas 扩展

`body_metas` 之前只为 top-level `HirItem::Fn` 解析符号名。Stage 5.6 扩展
为：当 owner 是 `HirItem::Impl` 时，遍历 impl items 找到匹配 body_id 的
方法，按 `landin_<Type>_<method>` 命名。

### 3.3 codegen `emit_vtables()`

新增 `pub fn emit_vtables(trait_resolver, interner, emitter)`，遍历
`trait_resolver.vtables`，对每个 (trait, type) pair 调用
`emitter.emit_vtable_global(global_name, method_symbols)`。

### 3.4 Emitter trait 新方法

新增 `fn emit_vtable_global(&mut self, global_name: &str, method_symbols: &[String]) -> EmitValue`。

发射格式（LLVM 15+ opaque pointer）：
```llvm
@.vtable.Foo.S = private unnamed_addr constant [1 x ptr] [ptr @landin_S_bar]
```

### 3.5 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `codegen::emit_vtables` | `emit_` 前缀 | 与 `emit_fat_ptr_type` 一致 |
| `Emitter::emit_vtable_global` | `emit_` 前缀 | 与 `emit_string_global` 一致 |
| `traits::extract_impl_self_ty_name` | snake_case + `_name` 后缀 | 与 `extract_*` 系列一致 |
| `VtableEntry.fn_name` | snake_case | 与 `BodyMeta.fn_name` 一致 |

## 4. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 5.6-a | `VtableEntry` 改造：`fn_def_id` → `fn_name` | L1 |
| 5.6-b | `collect()` 在构建 vtable entry 时解析符号名 | L2 |
| 5.6-c | `extract_impl_self_ty_name` 提升为 `pub` | L1 |
| 5.6-d | driver `body_metas` 扩展 impl 方法查找 | L2 |
| 5.6-e | `Emitter::emit_vtable_global` trait 方法 + TextEmitter 实现 | L2 |
| 5.6-f | `codegen::emit_vtables` 自由函数 | L1 |
| 5.6-g | `codegen_crate` 调用 `emit_vtables` | L1 |
| 5.6-h | lib.rs re-export `emit_vtables` + `extract_impl_self_ty_name` | L1 |
| 5.6-i | 测试 `vtable_codegen_tests.rs` (3 用例) | L1 |
| 5.6-j | Cargo.toml 版本 + all_tests.rs 模块注册 | L1 |

## 5. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（预期 919 + 3 = 922 passed）
4. §17.3 三阶段文档协议执行
5. §16 合规：codegen 仍是纯 MIR/TraitResolver 消费者
6. API 命名遵循 api-naming-standard §3

---

**创建日期**: 2026-07-22
