# Stage 5.14 测试计划：trait method query API

> **阶段**: Stage 5.14
> **对应代码**: tests/v0/stage5/plan/trait_method_query_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `trait_methods()`、`impl_methods()`、`trait_has_method()`、
`traits_with_method()`、`method_count_for_trait()` 正确查询 trait 方法。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| trait 方法列表 | test_trait_methods | ✅ | Foo 有 bar+baz |
| 未知 trait → None | test_trait_methods_unknown | ✅ | main 不是 trait |
| impl 方法列表 | test_impl_methods | ✅ | impl Foo for S 有 2 方法 |
| trait 有方法 | test_trait_has_method_true | ✅ | Foo 有 bar |
| trait 无方法 | test_trait_has_method_false | ✅ | Foo 无 "Foo" 方法 |
| 声明方法的 trait 列表 | test_traits_with_method | ✅ | 2 trait 声明 bar |
| trait 方法计数 | test_method_count_for_trait | ✅ | Foo 有 3 方法 |
| 未知 trait 计数=0 | test_method_count_for_trait_unknown | ✅ | main → 0 |

## 3. 测试统计

- 预期: 8, 实际: 8 (961 → 969, +8 ✅)

---

**创建日期**: 2026-07-22
