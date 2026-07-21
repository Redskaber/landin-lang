# mir_lowering 测试计划

> **阶段**: Stage 2
> **对应代码**: tests/v0/stage2/plan/mir_lowering_tests.rs
> **状态**: ✅ Complete
> **测试数**: 22

## 1. 测试目标

MIR lowering 测试 — 验证 HIR → MIR 转换 + 闭包 lowering。

## 2. 测试统计

- 实际测试数: 22
- 覆盖率: 100%

## 3. 迁移历史

Stage 4.8: 从扁平 `tests/*.rs` 迁移到标准化 `tests/v0/stage2/plan/` 目录结构（per v3.17 §17.1）。
