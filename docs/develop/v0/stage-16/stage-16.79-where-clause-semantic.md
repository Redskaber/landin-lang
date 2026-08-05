# Stage 16.79 — Where Clause Semantic Checking (Phase 2)

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.265.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

Per v0.4 roadmap, Where Clauses Phase 2: Full semantic checking (P2).

**前**: Phase 1 只验证 trait 是否存在，不验证类型是否实现 trait。
**后**: Phase 2 对具体类型（struct/enum）验证 trait 实现。

## 2. 设计-审查 Agent 循环 (§13.5)

1 轮自审定稿（scope 清晰，无 P0/P1 缺陷）：
- Design v1: `stage-16.79-where-clause-semantic-design.md`
- J1-J6 全部满足
- 设计决策：类型参数 T 推迟检查（Rust 语义——声明性约束）

## 3. 实现内容

### 3.1 扩展 check_where_clause_for_generics

**Phase 1** (保留): 验证 trait bound 引用存在的 trait
**Phase 2** (新增): 当 bounded type 是具体类型时，验证 trait 实现

```rust
fn check_where_clause_for_generics(
    generics: &HirGenerics,
    item_name: &str,
    resolver: &TraitResolver,  // 不再前缀 _
    interner: &Rodeo,
    errors: &mut Vec<TypeError>,
)
```

### 3.2 新增 resolve_bounded_type_def_id

```rust
fn resolve_bounded_type_def_id(bounded_ty: &HirTy) -> Option<DefId>
```

返回 `Some(def_id)` 仅当 bounded_ty 是 struct/enum；None 表示类型参数/原始类型/Self（推迟）。

### 3.3 新增 format_hir_ty_name + format_trait_name

用户友好的错误消息生成。

### 3.4 统一错误消息前缀

- `"where clause error: trait `{}` not found"` — Phase 1
- `"where clause error: type `{}` does not implement trait `{}`"` — Phase 2
- `"where clause error: `{}` is not a trait"` — bound 不是 trait

## 4. 测试计划 (§9.4.3 1:3+ ratio)

| # | 测试名 | 极性 | 描述 |
|---|--------|------|------|
| 1 | concrete_type_implements_trait | positive | S: Foo 且 S impl Foo → 无错误 |
| 2 | type_param_no_error | positive | T: Clone → 无错误（类型参数推迟） |
| 3 | concrete_struct_does_not_implement | negative | S: Foo 但 S 不 impl Foo → 错误 |
| 4 | concrete_enum_does_not_implement | negative | E: Foo 但 E 不 impl Foo → 错误 |
| 5 | multiple_bounds_one_unsatisfied | negative | S: Foo + Bar, 只 impl Foo → Bar 错误 |
| 6 | where_clause_on_other_struct | negative | A: Foo, A 不 impl → 错误 |
| 7 | trait_not_found_phase1_regression | negative | Phase 1 回归 |
| 8 | multiple_where_preds_one_fails | negative | 多 predicate, 一个失败 |

**比例**: 2:6 = 1:3 ✓

## 5. 验收 (§3.2)

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 365 lib + 2494 integration = 2859 unit tests |

## 6. 设计决策 (§14.8 偏差)

| 偏差类型 | 描述 | 处理 |
|---------|------|------|
| B1 | 类型参数 T 的 where clause 语义检查推迟 | 需要 trait solver（v0.5+），Rust 本身也不编译时检查 |
| B1 | Self 类型作为 bounded type 推迟 | 需要 trait/impl 上下文 |
| B1 | 原始类型（i32, bool）推迟 | 需要注册原始类型 trait impl |

## 7. 结论

GO — Where Clauses Phase 2 (concrete type impl verification) 完成：
- 具体类型（struct/enum）where clause 语义检查 ✅
- 类型参数推迟（Rust 语义）✅
- Phase 1 完全保留 ✅
- 8 新测试 1:3 正负比例 ✅

## 8. 后续工作

- Improved Error Messages (P3)
- Performance Optimization (P3)
- CodegenError error system (deferred from Stage 16.76)
- Type parameter where clause checking with trait solver (v0.5+)
