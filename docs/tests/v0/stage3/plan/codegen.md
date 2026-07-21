# codegen 测试计划

> **阶段**: Stage 3
> **对应代码**: tests/v0/stage3/plan/codegen_tests.rs
> **状态**: ✅ Complete
> **测试数**: 294

## 1. 测试目标

Codegen 测试 — 验证 MIR → LLVM IR 代码生成（所有 BinOp/UnOp/Aggregate/Cast/Projection/Terminator）。

## 2. 测试统计

- 实际测试数: 294
- 覆盖率: 100%

## 3. 迁移历史

Stage 4.8: 从扁平 `tests/*.rs` 迁移到标准化 `tests/v0/stage3/plan/` 目录结构（per v3.17 §17.1）。
