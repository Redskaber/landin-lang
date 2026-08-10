# Stage 17.05 — Trait Solver Phase 3 (Driver Integration)

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.278.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

将 Trait Solver 集成到 where clause 检查中，替代直接 resolver 调用。

## 2. 实现

`where_clause.rs` 的 `check_where_clause_for_generics` 中：
1. 创建 `TraitSolverCtxt::new(resolver, interner)`
2. 对具体类型 (Adt) 的 where clause bound，构建 `TraitPredicate` + `Goal`
3. 调用 `solver.evaluate(&goal)` 替代 `resolver.implements_by_def_ids`
4. 检查 `GoalEvaluationResult::No` → 报错

## 3. Trait Solver 基础架构完成

| Phase | Stage | 内容 |
|-------|-------|------|
| 1 | 17.03 | 数据结构 (TraitPredicate, Goal, GoalEvaluationResult, TraitSolverCtxt) |
| 2 | 17.04 | where clause assumptions (with_assumptions) |
| 3 | 17.05 | driver 集成 (where_clause.rs 使用 solver) |

## 4. 验收

| 命令 | 要求 | 实际 |
|------|------|------|
| cargo build --features llvm-backend | ✅ | ✅ |
| cargo fmt --check | ✅ | ✅ |
| cargo clippy --all-targets | 0 warnings | ✅ |
| cargo test | 0 failures | ✅ 431 lib + 2529 integration = 2960 |
