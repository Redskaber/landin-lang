# Stage 5.7 测试计划：dyn Trait fat-pointer 构造

> **阶段**: Stage 5.7
> **对应代码**: tests/v0/stage5/plan/dyn_trait_ptr_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `codegen_crate` 正确为每个 `impl Trait for Type` 构造 `dyn Trait`
fat-pointer 全局常量（`{ ptr, ptr }` — data + vtable）。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 单 dyn ptr 发射 | test_dyn_trait_ptr_emitted_for_impl | ✅ | `impl Foo for S` → IR 含 `@.dynptr.Foo.S` + `{ ptr, ptr }` |
| 无 impl 无 dyn ptr | test_no_dyn_trait_ptr_without_impl | ✅ | 无 impl → IR 不含任何 `@.dynptr.` |
| 多 dyn ptr 发射 | test_multiple_dyn_trait_ptrs_emitted | ✅ | 多 trait 多 impl → IR 含多个独立 dyn ptr 全局 |
| 类型构造函数 | test_emit_dyn_trait_ptr_type_shape | ✅ | `emit_dyn_trait_ptr_type()` 返回 `Struct([OpaquePtr, OpaquePtr])` |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 5.7 测试 |
|--------|----------------|
| 正面 | ✅ test_dyn_trait_ptr_emitted_for_impl |
| 负面 | ✅ test_no_dyn_trait_ptr_without_impl |
| 多态 | ✅ test_multiple_dyn_trait_ptrs_emitted |
| 单元 | ✅ test_emit_dyn_trait_ptr_type_shape |
| 集成 | ✅ 三个测试均通过 codegen_crate 入口 |

## 4. 测试统计

- 预期: 4, 实际: 4 (922 → 926, +4 ✅)

---

**创建日期**: 2026-07-22
