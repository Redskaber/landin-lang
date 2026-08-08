# Stage 17.04 — Trait Solver Phase 2 (Where Clause Assumptions)

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.277.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

为 Trait Solver 添加 where clause assumptions 支持。

## 2. 实现

### 2.1 TraitSolverCtxt 新增 assumptions 字段
`assumptions: Vec<(DefId, DefId)>` — (type_def_id, trait_def_id) 对。

### 2.2 with_assumptions() 构造函数
接受 where clause 提取的假设列表。

### 2.3 evaluate_implies() 优先检查 assumptions
对 Adt 类型先检查 assumptions，匹配则返回 Yes，否则 fallback 到 resolver。

## 3. 验收

| 命令 | 要求 | 实际 |
|------|------|------|
| cargo build --features llvm-backend | ✅ | ✅ |
| cargo fmt --check | ✅ | ✅ |
| cargo clippy --all-targets | 0 warnings | ✅ |
| cargo test | 0 failures | ✅ 431 lib + 2529 integration = 2960 |

## 4. 后续

Phase 3: driver integration — 从 where clause 提取 assumptions 并调用 solver。
