# Stage 18.74 — Incremental Compilation Phase 1 (Dependency Graph Infrastructure)

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.341.0 → v0.342.0
> **Process**: stage-committee-process.md v5.0 §13.1 (设计对齐) + §13.5 (设计-审查) + §14 (深度审查)
> **Status**: ✅ Complete

## 1. 背景

Stage 18.71-18.73 完成了所有 P0/P1 typeck 验证修复。按 v0.7 路线图，
下一优先级是**增量编译** (P1)。

增量编译分 3 个 Phase：
- **Phase 1**: 依赖图基础设施 (DependencyGraph + 依赖分析)
- **Phase 2**: 缓存键 (MIR hash → cache lookup)
- **Phase 3**: 增量重建 (只重编译变更的 items)

本 Stage 实现 Phase 1。

## 2. 设计方案

### 2.1 §1.0 原则应用

| 原则 | 应用 |
|------|------|
| 3 显式 > 隐式 | 依赖关系显式为 edge |
| 6 通用 > 特例 | 一个 graph 处理所有依赖类型 |
| 9 正确 > 妥协 | 依赖分析覆盖所有跨 item 引用 |

### 2.2 新模块: `src/incremental/`

```text
src/incremental/
├── mod.rs          # 模块入口 + re-exports
└── dep_graph.rs    # DependencyGraph + build_dependency_graph + compute_affected_items
```

### 2.3 DependencyGraph 结构

```rust
pub struct DependencyGraph {
    /// forward_edges: item → items it depends on
    forward_edges: HashMap<DefId, HashSet<DefId>>,
    /// reverse_edges: item → items that depend on it
    reverse_edges: HashMap<DefId, HashSet<DefId>>,
}

impl DependencyGraph {
    pub fn new() -> Self;
    pub fn add_edge(&mut self, from: DefId, to: DefId);
    pub fn dependencies(&self, item: DefId) -> Vec<DefId>;
    pub fn dependents(&self, item: DefId) -> Vec<DefId>;
    /// BFS on reverse edges — returns all items affected by changing `changed`
    pub fn compute_affected_items(&self, changed: &[DefId]) -> Vec<DefId>;
}
```

### 2.4 build_dependency_graph

分析 HIR 中的 6 种依赖类型：

| 依赖类型 | 示例 | 检测方式 |
|---------|------|---------|
| fn call | `fn a() { b() }` | `HirExprKind::Call` with `Res::Def` |
| type ref | `fn a() -> S` | `HirTyKind::Path` resolving to struct/enum |
| struct field | `fn a() { s.x }` | `HirExprKind::Field` on struct type |
| impl trait | `impl T for S` | `HirItem::Impl` with `of_trait` |
| impl self | `impl T for S` | `HirItem::Impl` with `self_ty` |
| supertrait | `trait T: U` | `HirTrait.supertraits` |

### 2.5 compute_affected_items

给定变更的 DefId 列表，通过 reverse_edges BFS 找出所有受影响的 items：
- 如果 `fn a` 变了，所有调用 `a` 的 fn 都受影响
- 如果 `struct S` 变了，所有引用 `S` 的 fn 都受影响
- BFS 遍历直到没有新的受影响 items

## 3. 测试矩阵

### 3.1 单元测试 (§9.4.3 1:3+ ratio)

| # | 测试 | 极性 | 描述 |
|---|------|------|------|
| 1 | dep_graph_empty | 正向 | 空 graph 无依赖 |
| 2 | dep_graph_single_edge | 正向 | 单条 edge 正确记录 |
| 3 | dep_graph_reverse_edge | 正向 | reverse_edges 自动维护 |
| 4 | dep_graph_affected_items | 正向 | BFS 找出受影响 items |
| 5 | dep_graph_no_cycle | 正向 | 循环依赖不会无限递归 |
| 6 | build_graph_fn_call | 负向 | fn call 依赖正确 |
| 7 | build_graph_type_ref | 负向 | type ref 依赖正确 |
| 8 | build_graph_struct_field | 负向 | struct field 依赖正确 |
| 9 | build_graph_impl_trait | 负向 | impl trait 依赖正确 |
| 10 | build_graph_supertrait | 负向 | supertrait 依赖正确 |

## 4. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | 增量编译是 v0.7 P1 路线图 |
| REV-A | GO | Phase 1 基础设施，无破坏性 |
| DEV-A | GO | 独立模块，不影响现有流水线 |
| QA-A | GO | 10 个单元测试 (1:3+ ratio) |
| PM-A | GO | 开发效率提升的基础 |

**5/5 GO** ✅
