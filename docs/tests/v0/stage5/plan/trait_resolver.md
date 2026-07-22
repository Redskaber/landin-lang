# Stage 5.1 测试计划：TraitResolver 基础

> **阶段**: Stage 5.1
> **对应代码**: tests/v0/stage5/plan/trait_resolver_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标
验证 TraitResolver 正确收集 trait 定义 + impl 块 + 构建方法分派表。

## 2. 覆盖场景
| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| trait 定义收集 | test_trait_collected | ✅ PASS | `trait Foo { fn bar(); }` 被收集 |
| impl 块收集 | test_impl_collected | ✅ PASS | `impl Foo for S { fn bar() {} }` 被收集 |
| 方法分派表 | test_method_dispatch_table | ✅ PASS | dispatch table 有 1 条目 |

## 3. 测试统计
- 预期: 3, 实际: 3, 覆盖率: 100%

---

**最后更新**: 2026-07-22 (Stage 5.1 完成)
