# Stage 5.29 测试计划：stdlib layer query

> **阶段**: Stage 5.29
> **对应代码**: tests/v0/stage5/plan/stdlib_layer_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `StdlibLayer` 枚举 + `layer_for_name()` + `names_for_layer()` 正确查询
stdlib 层归属。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 |
|------|-----------|------|
| Core 层查询 | test_layer_for_name_core | ✅ |
| Alloc 层查询 | test_layer_for_name_alloc | ✅ |
| None 层查询 | test_layer_for_name_none | ✅ |
| Core 层名称列表 | test_names_for_layer_core | ✅ |
| Alloc 层名称列表 | test_names_for_layer_alloc | ✅ |
| None 层空列表 | test_names_for_layer_none | ✅ |
| 枚举相等比较 | test_stdlib_layer_equality | ✅ |

---

**创建日期**: 2026-07-23
