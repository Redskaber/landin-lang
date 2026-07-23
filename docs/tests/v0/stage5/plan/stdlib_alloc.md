# Stage 5.28 测试计划：stdlib alloc layer

> **阶段**: Stage 5.28
> **对应代码**: tests/v0/stage5/plan/stdlib_alloc_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 STDLIB_ALLOC_TYPES + STDLIB_ALLOC_TRAITS 正确注册并包含在
all_stdlib_type_names / all_stdlib_trait_names / register_stdlib / prelude 中。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 |
|------|-----------|------|
| alloc 类型 | test_stdlib_alloc_types | ✅ |
| alloc traits | test_stdlib_alloc_traits | ✅ |
| 类型名含 alloc | test_all_type_names_includes_alloc | ✅ |
| trait 名含 alloc | test_all_trait_names_includes_alloc | ✅ |
| alloc 类型 interned | test_alloc_types_interned | ✅ |
| alloc traits interned | test_alloc_traits_interned | ✅ |
| prelude 含 alloc | test_prelude_contains_alloc | ✅ |
| alloc 类型数 | test_alloc_type_count | ✅ |
| alloc trait 数 | test_alloc_trait_count | ✅ |

---

**创建日期**: 2026-07-23
