# Stage 5.9 测试计划：builtin Copy 激活 + 健全性修复

> **阶段**: Stage 5.9
> **对应代码**: tests/v0/stage5/plan/builtin_copy_activation_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `impl Copy for S` 无需 `trait Copy {}` 即可工作（builtin Copy 激活），
且 Adt without `impl Copy` 正确返回 NOT Copy（soundness 修复）。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| builtin Copy 无需 trait 定义 | test_builtin_copy_works_without_trait_def | ✅ | `impl Copy for S`（无 trait Copy {}）→ is_copy_builtin=true |
| 无 impl Copy → 非 Copy | test_no_copy_impl_means_not_copy | ✅ | `struct S;` → is_copy_builtin=false（soundness） |
| 用户定义 trait 仍兼容 | test_copy_works_with_user_trait_def | ✅ | `trait Copy {}` + `impl Copy for S` → true |
| 多类型选择性 Copy | test_copy_selective_per_type | ✅ | A 有 impl Copy → true；B 无 → false |
| 向后兼容 | test_is_copy_backward_compat | ✅ | is_copy() 与 is_copy_builtin() 结果一致 |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 5.9 测试 |
|--------|----------------|
| 正面 | ✅ test_builtin_copy_works_without_trait_def |
| 负面（soundness） | ✅ test_no_copy_impl_means_not_copy |
| 多态 | ✅ test_copy_selective_per_type |
| 向后兼容 | ✅ test_copy_works_with_user_trait_def + test_is_copy_backward_compat |

## 4. 测试统计

- 预期: 5, 实际: 5 (931 → 936, +5 ✅)
- 另有 1 个测试更新：test_adt_fallback_copy → test_adt_without_copy_impl_not_copy

---

**创建日期**: 2026-07-22
