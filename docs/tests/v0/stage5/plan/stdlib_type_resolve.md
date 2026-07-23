# Stage 5.34 测试计划：stdlib type resolution

> **阶段**: Stage 5.34
> **对应代码**: tests/v0/stage5/plan/stdlib_type_resolve_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `resolve_stdlib_type()` + `StdlibTypeKind` + 类型查询函数正确映射
stdlib 类型名到 kind。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 |
|------|-----------|------|
| 整数解析 | test_resolve_integers | ✅ |
| 浮点解析 | test_resolve_floats | ✅ |
| 其他原始 | test_resolve_other_primitives | ✅ |
| alloc 类型 | test_resolve_alloc_types | ✅ |
| std 类型 | test_resolve_std_types | ✅ |
| 未知 | test_resolve_unknown | ✅ |
| is_primitive_type | test_is_primitive_type | ✅ |
| integer_bit_width | test_integer_bit_width | ✅ |
| is_signed_integer | test_is_signed_integer | ✅ |
| is_unsigned_integer | test_is_unsigned_integer | ✅ |
| is_float_type | test_is_float_type | ✅ |

---

**创建日期**: 2026-07-23
