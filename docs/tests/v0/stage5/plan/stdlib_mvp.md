# Stage 5.25 测试计划：stdlib MVP

> **阶段**: Stage 5.25
> **对应代码**: tests/v0/stage5/plan/stdlib_mvp_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 stdlib 核心类型、ops/convert/iter trait 名、prelude 和 register_stdlib。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 |
|------|-----------|------|
| 核心类型 | test_stdlib_core_types | ✅ |
| ops traits | test_stdlib_ops_traits | ✅ |
| convert traits | test_stdlib_convert_traits | ✅ |
| iter traits | test_stdlib_iter_traits | ✅ |
| 所有 trait 名 | test_all_stdlib_trait_names | ✅ |
| 所有类型名 | test_all_stdlib_type_names | ✅ |
| 默认 prelude | test_default_prelude | ✅ |
| prelude 长度 | test_prelude_len | ✅ |
| register_stdlib | test_register_stdlib | ✅ |
| prelude contains false | test_prelude_contains_false | ✅ |

---

**创建日期**: 2026-07-23
