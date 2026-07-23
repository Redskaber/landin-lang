# Stage 5.35 测试计划：stdlib type layout

> **阶段**: Stage 5.35
> **对应代码**: tests/v0/stage5/plan/stdlib_layout_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `type_size_bytes()` + `type_alignment_bytes()` + `is_zero_sized_type()` +
`type_description()` 正确查询原始类型的 layout 信息。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 |
|------|-----------|------|
| 整数 size | test_type_size_bytes_integers | ✅ |
| float/bool size | test_type_size_bytes_floats_bool | ✅ |
| ZST size | test_type_size_bytes_zst | ✅ |
| None size | test_type_size_bytes_none | ✅ |
| alignment | test_type_alignment_bytes | ✅ |
| is_zero_sized | test_is_zero_sized_type | ✅ |
| description | test_type_description | ✅ |

---

**创建日期**: 2026-07-23
