# Stage 5.17 测试计划：vtable method resolution

> **阶段**: Stage 5.17
> **对应代码**: tests/v0/stage5/plan/vtable_method_resolve_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `resolve_vtable_method()`、`vtable_method_names()`、
`vtable_has_method()` 正确解析 vtable 方法到具体 LLVM 符号名。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 解析方法符号 | test_resolve_vtable_method | ✅ | bar → landin_S_bar |
| 未知方法 → None | test_resolve_vtable_method_unknown_method | ✅ | "Foo" 方法 → None |
| 无 impl → None | test_resolve_vtable_method_no_impl | ✅ | 无 impl → None |
| 所有方法符号 | test_vtable_method_names | ✅ | 2 方法 → [landin_S_bar, landin_S_baz] |
| 无 vtable → 空 | test_vtable_method_names_empty | ✅ | 无 impl → [] |
| vtable 有方法 | test_vtable_has_method_true | ✅ | bar → true |
| vtable 无方法 | test_vtable_has_method_false | ✅ | "Foo" → false |
| 多方法解析 | test_resolve_multiple_methods | ✅ | bar/baz/qux 全部解析 |

## 3. 测试统计

- 预期: 8, 实际: 8 (984 → 992, +8 ✅)

---

**创建日期**: 2026-07-22
