# Stage 16.79 测试计划 — Where Clause Semantic Checking (Phase 2)

> **阶段**: Stage 16.79
> **对应代码**: src/typeck/where_clause.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 where clause 语义检查（Phase 2）正确工作，Phase 1 回归通过。

## 2. 覆盖场景

| 场景 | 测试函数名 | 极性 | 状态 | 说明 |
|------|-----------|------|------|------|
| 具体类型实现 trait | stage16_79_concrete_type_implements_trait | positive | ✅ PASS | S impl Foo → 无错误 |
| 类型参数无错误 | stage16_79_type_param_no_error | positive | ✅ PASS | T: Clone → 无错误（推迟） |
| struct 不实现 trait | stage16_79_concrete_struct_does_not_implement | negative | ✅ PASS | S 不 impl Foo → 错误 |
| enum 不实现 trait | stage16_79_concrete_enum_does_not_implement | negative | ✅ PASS | E 不 impl Foo → 错误 |
| 多 bound 一个不满足 | stage16_79_multiple_bounds_one_unsatisfied | negative | ✅ PASS | S: Foo+Bar, 只 impl Foo → Bar 错误 |
| 其他 struct 不实现 | stage16_79_where_clause_on_other_struct | negative | ✅ PASS | A: Foo, A 不 impl → 错误 |
| Phase 1 回归 | stage16_79_trait_not_found_phase1_regression | negative | ✅ PASS | trait 不存在仍报错 |
| 多 predicate 一个失败 | stage16_79_multiple_where_preds_one_fails | negative | ✅ PASS | 多 predicate, 一个失败 |
| Phase 1 全部回归 | stage16_73_* (5 tests) | both | ✅ PASS | 全部通过 |

## 3. 测试统计

- 预期测试数: 13 (8 new + 5 existing Phase 1)
- 实际测试数: 13
- 新增正向: 2
- 新增负向: 6
- 新增比例: 2:6 = 1:3 ✓
- 覆盖率: 100%

## 4. 依赖

- Stage 16.73 (Phase 1: trait existence)
- Stage 16.07-16.11 (Task 3: TraitResolver DefId-keyed lookup)

## 5. 结论

全部 8 个新测试通过，5 个 Phase 1 测试回归通过，2859 全量测试 0 failures。
