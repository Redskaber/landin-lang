# Stage 5.8 测试计划：标准 trait 注册表（stdlib MVP）

> **阶段**: Stage 5.8
> **对应代码**: tests/v0/stage5/plan/builtin_traits_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证编译器自动识别 10 个内置标准 trait（Copy, Clone, Drop, Sized, Send,
Sync, Unpin, Fn, FnMut, FnOnce），无需用户定义。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 内置 trait 全部注册 | test_builtin_traits_registered | ✅ | 10 个 trait 都在 builtin_traits 中 |
| DefId 在保留范围 | test_builtin_trait_def_ids_in_reserved_range | ✅ | DefId ∈ [u32::MAX-9, u32::MAX] |
| 用户 trait 非内置 | test_user_defined_trait_not_builtin | ✅ | `trait Foo` → is_builtin_trait=false |
| 内置+用户定义共存 | test_builtin_copy_recognized_even_with_user_definition | ✅ | `trait Copy {}` → 仍识别为内置 |
| 内置 trait 数量 | test_builtin_trait_count | ✅ | builtin_traits.len() == 10 |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 5.8 测试 |
|--------|----------------|
| 正面 | ✅ test_builtin_traits_registered |
| 负面 | ✅ test_user_defined_trait_not_builtin |
| 边界 | ✅ test_builtin_copy_recognized_even_with_user_definition |
| 单元 | ✅ test_builtin_trait_def_ids_in_reserved_range + test_builtin_trait_count |

## 4. 测试统计

- 预期: 5, 实际: 5 (926 → 931, +5 ✅)

---

**创建日期**: 2026-07-22
