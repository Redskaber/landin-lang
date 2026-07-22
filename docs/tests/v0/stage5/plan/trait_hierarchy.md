# Stage 5.15 测试计划：trait hierarchy（supertraits）

> **阶段**: Stage 5.15
> **对应代码**: tests/v0/stage5/plan/trait_hierarchy_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `trait_supertraits()`、`trait_has_supertrait()`、
`supertrait_count_for_trait()` 正确查询 supertrait 信息。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| supertrait 列表 | test_trait_supertraits | ✅ | `trait Foo: Bar` → [Bar] |
| 无 supertrait | test_trait_supertraits_empty | ✅ | `trait Foo` → [] |
| 未知 trait → None | test_trait_supertraits_unknown | ✅ | main → None |
| 有 supertrait | test_trait_has_supertrait_true | ✅ | Foo 有 Bar |
| 无 supertrait | test_trait_has_supertrait_false | ✅ | Foo 无 Baz |
| supertrait 计数 | test_supertrait_count_for_trait | ✅ | `C: A + B` → 2 |
| 零 supertrait | test_supertrait_count_for_trait_zero | ✅ | Foo → 0 |
| 多 supertrait | test_multiple_supertraits | ✅ | `D: A + B + C` → 3 |

## 3. 测试统计

- 预期: 8, 实际: 8 (969 → 977, +8 ✅)

---

**创建日期**: 2026-07-22
