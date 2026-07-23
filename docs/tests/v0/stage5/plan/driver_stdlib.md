# Stage 5.26 测试计划：driver stdlib integration

> **阶段**: Stage 5.26
> **对应代码**: tests/v0/stage5/plan/driver_stdlib_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 driver 调用 register_stdlib() 后所有 stdlib 名称已 interned，
CompileResult.stdlib_prelude 已填充。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 |
|------|-----------|------|
| prelude 填充 | test_stdlib_prelude_populated | ✅ |
| 类型 interned | test_stdlib_types_interned | ✅ |
| ops traits interned | test_stdlib_ops_traits_interned | ✅ |
| convert traits interned | test_stdlib_convert_traits_interned | ✅ |
| iter traits interned | test_stdlib_iter_traits_interned | ✅ |
| prelude 含类型 | test_prelude_contains_types | ✅ |
| prelude 含 traits | test_prelude_contains_traits | ✅ |
| lex error 路径 | test_stdlib_prelude_on_lex_error | ✅ |

---

**创建日期**: 2026-07-23
