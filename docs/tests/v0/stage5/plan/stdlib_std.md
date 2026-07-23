# Stage 5.30 测试计划：stdlib std layer

> **阶段**: Stage 5.30
> **对应代码**: tests/v0/stage5/plan/stdlib_std_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 STDLIB_STD_TYPES + STDLIB_STD_TRAITS + StdlibLayer::Std 正确注册和查询。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 |
|------|-----------|------|
| std 类型存在 | test_std_types_present | ✅ |
| std traits 存在 | test_std_traits_present | ✅ |
| std 类型 interned | test_std_types_interned | ✅ |
| std traits interned | test_std_traits_interned | ✅ |
| layer_for_name Std | test_layer_for_name_std | ✅ |
| names_for_layer Std | test_names_for_layer_std | ✅ |
| Std 层唯一性 | test_stdlib_layer_std_distinct | ✅ |
| prelude 含 std | test_prelude_contains_std | ✅ |

---

**创建日期**: 2026-07-23
