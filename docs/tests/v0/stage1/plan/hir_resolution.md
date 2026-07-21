# hir_resolution 测试计划

> **阶段**: Stage 1
> **对应代码**: tests/v0/stage1/plan/hir_resolution_tests.rs
> **状态**: ✅ Complete
> **测试数**: 26

## 1. 测试目标

名称解析测试 — 验证模块级路径解析 + use 声明解析 + 可见性 + 嵌套模块。

## 2. 测试统计

- 实际测试数: 26
- 覆盖率: 100%

## 3. 迁移历史

Stage 4.8: 从扁平 `tests/*.rs` 迁移到标准化 `tests/v0/stage1/plan/` 目录结构（per v3.17 §17.1）。
