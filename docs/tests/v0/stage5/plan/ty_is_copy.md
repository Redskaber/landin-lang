# Stage 5.3 测试计划：改进 ty_is_copy

> **阶段**: Stage 5.3
> **对应代码**: tests/v0/stage5/plan/ty_is_copy_tests.rs
> **状态**: 🔄 In progress

## 1. 测试目标

验证 `ty_is_copy_with_resolver` 正确检测 Copy trait 实现。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 基本类型始终 Copy | test_primitives_always_copy | ⏳ | i32/bool/f64 无需 resolver |
| 无 Copy impl 的 Adt | test_adt_without_copy_impl | ⏳ | 无 impl Copy → 非 Copy |
| 有 Copy impl 的 Adt | test_adt_with_copy_impl | ⏳ | impl Copy → Copy |

## 3. 测试统计

- 预期: 3, 实际: TBD

---

**创建日期**: 2026-07-22
