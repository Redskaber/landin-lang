# Stage 6 — Test Documentation

> **阶段范围**: Stage 6.1 - 6.18 (18 sub-stages, architectural refactoring)
> **测试目录**: `tests/v0/stage6/plan/`
> **状态**: ✅ Complete

## 测试范围说明

Stage 6 是**纯架构重构阶段** (TD-011/017/022-026) — 所有拆分都是
behavior-equivalent，未新增功能，未新增测试。Stage 6.1-6.18 全程保持 1881 tests
不变。

## 测试目录结构

```
tests/v0/stage6/
└── plan/
    └── README.md  ← 本文件 (placeholder; Stage 6 无新增测试)
```

## 回归验证策略

虽然 Stage 6 无新增测试，但每轮重构均通过 §1.2 验收 (cargo test 全绿) 验证
behavior-equivalence：

| 子阶段 | 重构对象 | LOC 变化 | 1881 tests |
|--------|---------|---------|-----------|
| 6.1 | mir/lower adt_layout split | -153 | ✅ pass |
| 6.2 | mir/lower closure_capture split | -158 | ✅ pass |
| 6.3 | mir/lower pattern_bindings split | -305 | ✅ pass |
| 6.4 | mir/lower overflow_assert split | -74 | ✅ pass |
| 6.5 | mir/lower field_resolution split | -204 | ✅ pass |
| 6.6 | mir/lower control_flow split | -472 | ✅ pass |
| 6.7 | codegen trait_dispatch split | -949 | ✅ pass |
| 6.8 | codegen mir_translation split | -462 | ✅ pass |
| 6.9 | stdlib 3-domain split | restructure | ✅ pass |
| 6.10 | mir/lower expr_operand split | -1208 | ✅ pass |
| 6.11 | process v3.21 (no code change) | 0 | ✅ pass |
| 6.12 | parser.rs split (-2849 LOC) | -2849 | ✅ pass |
| 6.13 | lexer/reader.rs split (-1188 LOC) | -1188 | ✅ pass |
| 6.14 | borrowck/mod.rs split | -306 | ✅ pass |
| 6.15 | typeck/checker.rs split | -160 | ✅ pass |
| 6.16 | resolve/resolver.rs split | -977 | ✅ pass |
| 6.17 | mir/lower expr_operand sub-split (REVERTED in 6.18) | -180 | ✅ pass |
| 6.18 | Stage 6 finale: revert 6.17 + §25.8 writeback | +180 | ✅ pass |

## 累计 LOC 减少

- Stage 6 总计: ~9200+ LOC 减少 (across 47 module splits)
- 最大单文件: < 1500 LOC (borrowck/region_inference.rs 1462 LOC, incl. tests)

## 关联文档

- `docs/develop/v0/stage-6/README.md` — Stage 6 开发文档索引
- `docs/develop/v0/stage-6/plan-6.{1..18}.md` — 各子阶段开发计划
- `docs/develop/v0/stage-6/gate-review-6.{1..18}.md` — 各子阶段门审查
