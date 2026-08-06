# Stage 16.85 测试计划 — Migrate expr_operand.rs Type Errors to Use Resolver

> **阶段**: Stage 16.85
> **对应代码**: src/mir/lower/mod.rs + src/mir/lower/expr_operand.rs + src/borrowck/mod.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 MIR lower "no method found" 错误使用 resolver-backed 类型名。

## 2. 覆盖场景

| 场景 | 测试函数名 | 极性 | 状态 | 说明 |
|------|-----------|------|------|------|
| no method found | stage15_88_no_method_found_uses_human_readable_type_name | negative | ✅ PASS | 显示 "S" 而非 "<adt>" |
| 全量回归 | cargo test | both | ✅ PASS | 2944 tests, 0 failures |

## 3. 测试统计

- 预期测试数: 1 (回归更新) + 全量回归
- 实际测试数: 415 lib + 2529 integration = 2944
- 覆盖率: 100%

## 4. 依赖

- Stage 16.80 (type_kind_to_string_with_resolver)
- Stage 16.07-16.11 (Task 3: TraitResolver DefId-keyed lookup)

## 5. 结论

stage15_88 回归测试更新通过，2944 全量测试 0 failures，0 warnings。
