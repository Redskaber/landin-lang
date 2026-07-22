# Stage 5.6 测试计划：vtable codegen 发射

> **阶段**: Stage 5.6
> **对应代码**: tests/v0/stage5/plan/vtable_codegen_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `codegen_crate` 正确将 TraitResolver 中的 vtable 数据发射为 LLVM IR
全局常量。每个 `impl Trait for Type` 应产生一个 `@.vtable.<trait>.<type>`
全局，其类型为 `[N x ptr]`，每个 `ptr` 指向对应 impl 方法的符号
(`landin_<Type>_<method>`)。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 单 vtable 发射 | test_vtable_global_emitted_for_impl | ✅ | `impl Foo for S` → IR 含 `@.vtable.Foo.S` + `@landin_S_bar` |
| 无 impl 无 vtable | test_no_vtable_global_without_impl | ✅ | 无 impl → IR 不含任何 `@.vtable.` |
| 多 vtable 发射 | test_multiple_vtable_globals_emitted | ✅ | 多 trait 多 impl → IR 含多个独立 vtable 全局 |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 5.6 测试 |
|--------|----------------|
| 正面 | ✅ test_vtable_global_emitted_for_impl |
| 负面 | ✅ test_no_vtable_global_without_impl |
| 多态 | ✅ test_multiple_vtable_globals_emitted |
| 集成 | ✅ 三个测试均通过 codegen_crate 入口 |

## 4. 测试统计

- 预期: 3, 实际: 3 (919 → 922, +3 ✅)

---

**创建日期**: 2026-07-22
