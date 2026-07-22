# Stage 5.18 测试计划：trait coherence checking

> **阶段**: Stage 5.18
> **对应代码**: tests/v0/stage5/plan/trait_coherence_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `check_coherence()`、`has_coherence_error()`、
`coherence_error_count()` 正确检测 conflicting impls。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 无冲突 | test_check_coherence_no_conflicts | ✅ | 不同 trait 的 impl → 空 Vec |
| 检测冲突 | test_check_coherence_detects_conflict | ✅ | 同 trait+type 2 impl → 1 error |
| 有冲突 | test_has_coherence_error_true | ✅ | (Foo, S) 冲突 → true |
| 无冲突 | test_has_coherence_error_false | ✅ | 单 impl → false |
| 冲突计数 | test_coherence_error_count | ✅ | 2 对冲突 → 2 |
| 零冲突 | test_coherence_error_count_zero | ✅ | 单 impl → 0 |
| 不同类型无冲突 | test_no_conflict_different_types | ✅ | 同 trait 不同 type → 0 |

## 3. 测试统计

- 预期: 7, 实际: 7 (992 → 999, +7 ✅)

---

**创建日期**: 2026-07-22
