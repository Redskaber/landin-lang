# Stage 5.13 测试计划：trait impl 统计

> **阶段**: Stage 5.13
> **对应代码**: tests/v0/stage5/plan/trait_impl_stats_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `impl_count_for_type()`、`impl_count_for_trait()`、
`builtin_trait_count()`、`traits_for_type()` 正确统计和列举 trait 实现。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 类型 impl 计数 | test_impl_count_for_type | ✅ | S 有 2 个 impl → 2 |
| 类型零 impl | test_impl_count_for_type_zero | ✅ | S 无 impl → 0 |
| trait impl 计数 | test_impl_count_for_trait | ✅ | Foo 有 2 个 impl → 2 |
| trait 零 impl | test_impl_count_for_trait_zero | ✅ | Foo 无 impl → 0 |
| 内置 trait 计数 | test_builtin_trait_count | ✅ | 10 个 builtin |
| 类型 trait 列表 | test_traits_for_type | ✅ | S 实现 Foo+Bar → 包含两者 |
| 类型空 trait 列表 | test_traits_for_type_empty | ✅ | S 无 impl → 空 Vec |

## 3. 测试统计

- 预期: 7, 实际: 7 (954 → 961, +7 ✅)

---

**创建日期**: 2026-07-22
