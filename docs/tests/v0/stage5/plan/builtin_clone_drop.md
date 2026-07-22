# Stage 5.10 测试计划：builtin Clone/Drop 激活 + 通用 builtin trait 检查

> **阶段**: Stage 5.10
> **对应代码**: tests/v0/stage5/plan/builtin_clone_drop_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `is_clone_builtin()`、`is_drop_builtin()`、`implements_builtin_trait()`
正确检测 builtin trait impls（无需用户定义 trait）。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| builtin Clone 激活 | test_builtin_clone_works_without_trait_def | ✅ | `impl Clone for S`（无 trait 定义）→ true |
| builtin Drop 激活 | test_builtin_drop_works_without_trait_def | ✅ | `impl Drop for S`（无 trait 定义）→ true |
| 无 impl Clone → 非 Clone | test_no_clone_impl_means_not_clone | ✅ | `struct S;` → false |
| 通用 Copy 检查 | test_generic_builtin_trait_check_copy | ✅ | implements_builtin_trait("Copy") → true |
| 通用 Clone 检查 | test_generic_builtin_trait_check_clone | ✅ | implements_builtin_trait("Clone") → true |
| 通用 false 检查 | test_generic_builtin_trait_check_false | ✅ | implements_builtin_trait("Drop") 无 impl → false |
| 多 trait 同类型 | test_multiple_builtin_traits_on_same_type | ✅ | Copy+Clone+Drop 同时 impl → 全 true |

## 3. 测试统计

- 预期: 7, 实际: 7 (936 → 943, +7 ✅)

---

**创建日期**: 2026-07-22
