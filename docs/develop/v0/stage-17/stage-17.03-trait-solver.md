# Stage 17.03 — Trait Solver Phase 1 (Data Structures)

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.276.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

定义 Trait Solver 核心数据结构，为 Phase 2 solving 算法做准备。

## 2. 实现

### 2.1 TraitPredicate
`{ ty: Ty, trait_def_id: DefId }` — "Type: Trait" 断言。

### 2.2 Goal
`enum Goal { Implies(TraitPredicate) }` — solver 评估目标。

### 2.3 GoalEvaluationResult
`enum { Yes, No, Ambiguous }` — 评估结果。

### 2.4 TraitSolverCtxt
`{ resolver: &TraitResolver, interner: &Rodeo }` — solver 上下文。

### 2.5 evaluate() stub
- 具体类型 (Adt) → `resolver.implements_by_def_ids` → Yes/No
- 类型参数 (Param) → Ambiguous
- 推断变量 (Infer) → Ambiguous
- Error 类型 → Yes (suppressed)

## 3. 测试 (§9.4.3 1:3)

| # | 测试 | 极性 |
|---|------|------|
| 1 | trait_predicate_construction | positive |
| 2 | goal_evaluation_concrete_type_implements | positive |
| 3 | concrete_type_not_implements | negative |
| 4 | type_param_ambiguous | negative |
| 5 | infer_var_ambiguous | negative |
| 6 | error_type_yes | negative |
| 7 | trait_solver_ctxt_new | negative |
| 8 | goal_implies_variant | negative |

## 4. 验收

| 命令 | 要求 | 实际 |
|------|------|------|
| cargo build --features llvm-backend | ✅ | ✅ |
| cargo fmt --check | ✅ | ✅ |
| cargo clippy --all-targets | 0 warnings | ✅ |
| cargo test | 0 failures | ✅ 431 lib + 2529 integration = 2960 |

## 5. 后续

Phase 2: where clause assumptions + supertrait expansion + driver integration.
