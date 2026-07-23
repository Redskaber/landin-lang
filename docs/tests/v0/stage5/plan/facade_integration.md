# Stage 5.33 测试计划：stdlib facade driver integration

> **阶段**: Stage 5.33
> **对应代码**: tests/v0/stage5/plan/facade_integration_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `CompileResult.stdlib_facade` 正确填充并可查询。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 |
|------|-----------|------|
| facade 填充 | test_facade_populated | ✅ |
| type_count | test_facade_type_count_in_result | ✅ |
| is_stdlib_name | test_facade_is_stdlib_name_in_result | ✅ |
| summary | test_facade_summary_in_result | ✅ |
| type_count_for_layer | test_facade_type_count_for_layer_in_result | ✅ |
| lex error 路径 | test_facade_on_lex_error | ✅ |
| trait_count | test_facade_trait_count_in_result | ✅ |

---

**创建日期**: 2026-07-23
