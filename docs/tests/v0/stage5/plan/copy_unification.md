# Stage 5.12 测试计划：Copy 检测统一化

> **阶段**: Stage 5.12
> **对应代码**: tests/v0/stage5/plan/copy_unification_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `ty_is_copy_unified()` 正确委托给 `ty_is_copy_with_resolver`，且
primitive 分支通过 `is_primitive_copy_kind()` 检查。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| primitive 是 Copy | test_unified_primitive_is_copy | ✅ | i32 → true |
| unified 与 with_resolver 一致 | test_unified_matches_with_resolver | ✅ | 5 种 TyKind 结果一致 |
| Adt 无 impl Copy → 非 Copy | test_unified_adt_without_copy_not_copy | ✅ | Adt → false |
| 集成：impl Copy → Copy | test_unified_integration_with_impl_copy | ✅ | compile + unified → true |
| 集成：无 impl Copy → 非 Copy | test_unified_integration_without_impl_copy | ✅ | compile + unified → false |

## 3. 测试统计

- 预期: 5, 实际: 5 (949 → 954, +5 ✅)

---

**创建日期**: 2026-07-22
