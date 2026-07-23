# Stage 5.31 测试计划：stdlib facade

> **阶段**: Stage 5.31
> **对应代码**: tests/v0/stage5/plan/stdlib_facade_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `StdlibFacade` 聚合统计 + 层查询正确。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 |
|------|-----------|------|
| 类型总数 | test_facade_type_count | ✅ |
| trait 总数 | test_facade_trait_count | ✅ |
| 层类型数 | test_facade_type_count_for_layer | ✅ |
| 层数 | test_facade_layer_count | ✅ |
| is_stdlib_name | test_facade_is_stdlib_name | ✅ |
| summary | test_facade_summary | ✅ |
| from_prelude | test_facade_from_prelude | ✅ |
| from_compile_result | test_facade_from_compile_result | ✅ |

---

**创建日期**: 2026-07-23
