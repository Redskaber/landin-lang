# integration 测试计划

> **阶段**: Stage 2
> **对应代码**: tests/v0/stage2/plan/integration_tests.rs
> **状态**: ✅ Complete
> **测试数**: 58

## 1. 测试目标

集成测试 — 端到端流水线验证（lexer→parser→HIR→resolve→MIR→typeck→borrowck）。

## 2. 测试统计

- 实际测试数: 58
- 覆盖率: 100%

## 3. 迁移历史

Stage 4.8: 从扁平 `tests/*.rs` 迁移到标准化 `tests/v0/stage2/plan/` 目录结构（per v3.17 §17.1）。
