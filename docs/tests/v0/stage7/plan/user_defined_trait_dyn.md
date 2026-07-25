# Stage 7.6 测试计划: User-defined trait dyn support (TD-018)

> **阶段**: Stage 7.6
> **对应代码**: tests/v0/stage7/plan/user_defined_trait_dyn_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 TD-018 user-defined trait dyn 支持 — `dyn Trait` 不仅限于 stdlib traits,
还能用于用户自定义的 trait。通过 `build_dyn_trait_method_calls_from_resolver`
统一处理 stdlib + user-defined traits。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| Fat ptr 生成 | test_user_trait_dyn_fat_ptr_generation | ✅ | user-defined trait 生成 DynTraitFatPtr |
| Method calls from resolver | test_user_trait_dyn_method_calls_from_resolver | ✅ | 通过 TraitResolver.vtables 查找 method |
| Slot index 排序 (0,1,2) | test_user_trait_dyn_slot_index_ordering | ✅ | vtable slot 按 method 声明顺序 0-indexed |
| 空 methods | test_user_trait_dyn_empty_methods | ✅ | trait 无 method 时 vtable 为空 |
| 多 trait | test_user_trait_dyn_multiple_traits | ✅ | 不同 trait 各自生成 vtable |
| stdlib 回归 | test_user_trait_dyn_stdlib_regression | ✅ | stdlib trait 仍走旧路径 |
| Method call 字段 | test_user_trait_dyn_method_call_fields | ✅ | DynTraitMethodCall 字段正确 |
| 多类型同 trait | test_user_trait_dyn_multiple_types_same_trait | ✅ | 多个 impl 同一 trait 各自 vtable |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 7.6 测试 |
|--------|----------------|
| 正面 (user-defined) | ✅ test_user_trait_dyn_fat_ptr_generation |
| 回归 (stdlib) | ✅ test_user_trait_dyn_stdlib_regression |
| 边界 (空 methods) | ✅ test_user_trait_dyn_empty_methods |
| 多态 (多类型同 trait) | ✅ test_user_trait_dyn_multiple_types_same_trait |
| 一致性 (slot ordering) | ✅ test_user_trait_dyn_slot_index_ordering |

## 4. 测试统计

- 预期: 8, 实际: 8 (2015 → 2023, +8 ✅)
- TD-018 complete: user-defined trait dyn support

---

**创建日期**: 2026-07-25
