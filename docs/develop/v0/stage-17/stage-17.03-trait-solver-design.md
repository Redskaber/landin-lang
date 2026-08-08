# Stage 17.03 Design — Trait Solver Phase 1 (Data Structures)

> **Author**: ARCH-A (Design Agent) + REV-A (self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — scope clear, self-reviewed)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审)

## 1. 阶段目标

Per v0.5 roadmap, Trait Solver Phase 1: 定义数据结构。

**问题**: 当前 `where T: Clone` 是声明性约束，不验证。Trait solver 将在 typeck 时检查具体类型是否实现 trait bound。

**Phase 1 目标**: 定义 `TraitPredicate`, `Goal`, `GoalEvaluationResult`, `TraitSolverCtxt` 数据结构，为 Phase 2 的 solving 算法做准备。

## 2. 架构现状

### 2.1 现有 TraitResolver

```rust
pub struct TraitResolver {
    pub trait_by_name: HashMap<Spur, DefId>,
    pub type_by_def_id: HashMap<DefId, Spur>,
    // ...
    pub fn implements_by_def_ids(&self, trait_def_id: DefId, self_type_def_id: DefId) -> bool
}
```

已有 DefId-keyed 的 trait impl 查询，但只能查**具体类型**（DefId → DefId）。不能查：
- 类型参数 T（无 DefId）是否实现某 trait
- 泛型实例化后的类型是否实现某 trait

### 2.2 现有 where clause 检查

`check_where_clauses` 只验证 trait 存在 + 具体类型是否实现。类型参数 T 跳过（声明性约束）。

## 3. 数据结构设计

### 3.1 TraitPredicate

```rust
/// A trait predicate: "Type: Trait"
/// Represents a claim that `ty` implements `trait_def_id`.
#[derive(Debug, Clone)]
pub struct TraitPredicate {
    /// The type that should implement the trait.
    pub ty: Ty,
    /// The DefId of the trait.
    pub trait_def_id: DefId,
}
```

### 3.2 Goal

```rust
/// A goal to be evaluated by the trait solver.
/// Currently only supports "does type T implement trait X?"
#[derive(Debug, Clone)]
pub enum Goal {
    /// Prove that `ty` implements `trait_def_id`.
    Implies(TraitPredicate),
}
```

### 3.3 GoalEvaluationResult

```rust
/// The result of evaluating a goal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalEvaluationResult {
    /// The goal is provably true.
    Yes,
    /// The goal is provably false.
    No,
    /// The goal cannot be determined yet (e.g., type is an inference variable).
    Ambiguous,
}
```

### 3.4 TraitSolverCtxt

```rust
/// Context for the trait solver.
/// Holds a reference to TraitResolver for impl lookup.
pub struct TraitSolverCtxt<'a> {
    pub resolver: &'a TraitResolver,
    pub interner: &'a Rodeo,
}

impl<'a> TraitSolverCtxt<'a> {
    pub fn new(resolver: &'a TraitResolver, interner: &'a Rodeo) -> Self { ... }

    /// Evaluate a goal. Phase 1: stub that returns Ambiguous for type params,
    /// delegates to resolver.implements_by_def_ids for concrete types.
    pub fn evaluate(&self, goal: &Goal) -> GoalEvaluationResult { ... }
}
```

## 4. J1-J6

| # | 判据 | 满足 |
|---|------|------|
| J1 | 对齐 04-type-system.md | ✅ |
| J2 | 单一职责 | ✅ solver 只负责 goal evaluation |
| J3 | 单向流动 | ✅ solver → resolver → result |
| J4 | 完整 | ✅ 4 个核心类型 |
| J5 | 阶段清晰 | ✅ typeck/ 模块 |
| J6 | 粒度 | ✅ ~80 LOC |

## 5. 测试 (§9.4.3 1:3+)

### positive (2)
1. `trait_predicate_construction` — TraitPredicate 正确构造
2. `goal_evaluation_concrete_type_implements` — 具体类型实现 trait → Yes

### negative (6)
1. `goal_evaluation_concrete_type_not_implements` — 具体类型不实现 → No
2. `goal_evaluation_type_param_ambiguous` — 类型参数 → Ambiguous
3. `goal_evaluation_infer_var_ambiguous` — Infer 变量 → Ambiguous
4. `goal_evaluation_error_type_yes` — Error 类型 → Yes (suppressed)
5. `trait_solver_ctxt_new` — 构造正确
6. `goal_implies_variant` — Goal::Implies 变体正确

比例: 2:6 = 1:3 ✓

## 6. 验收

- cargo build --features llvm-backend ✅
- cargo fmt --check ✅
- cargo clippy --all-targets 0 warnings ✅
- cargo test 0 failures ✅

## 7. 结论

定稿。Phase 1 ~80 LOC + 8 测试。
