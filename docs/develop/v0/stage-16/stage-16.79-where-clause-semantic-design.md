# Stage 16.79 Design — Where Clause Semantic Checking (Phase 2)

> **Author**: ARCH-A (Design Agent) + REV-A (self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — scope clear, self-reviewed)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审)

## 1. 阶段目标

Per v0.4 roadmap, Where Clauses Phase 2: Full semantic checking (P2, 2-3 stages).

**当前状态** (Stage 16.73 Phase 1): `check_where_clauses` 只验证 trait 是否存在（`Res::Def` vs `Res::Unknown`），不验证类型是否真正实现了 trait。

**问题**: `fn f<T>() where T: NonExistentTrait` 会报错（Phase 1），但 `fn f(x: Foo) where Foo: NonImplementedTrait` 不会报错——即使 `Foo` 没有实现 `NonImplementedTrait`。

**目标**: 扩展 `check_where_clauses` 验证当 bounded type 是**具体类型**（struct/enum）时，该类型是否真正实现了 bound trait。

## 2. 架构现状分析

### 2.1 当前 where_clause.rs 结构

```rust
pub fn check_where_clauses(hir, resolver, interner) -> Vec<TypeError>
fn check_where_clause_for_generics(generics, item_name, _resolver, interner, errors)
```

**问题**:
- `_resolver` 参数未使用（前缀 `_` 标记）— Phase 1 不需要 TraitResolver
- 只检查 `tc.path.res` 是 `Res::Def` 还是 `Res::Unknown`，不检查 `bounded_ty` 的实现

### 2.2 TraitResolver 能力

```rust
pub fn implements_by_def_ids(&self, trait_def_id: DefId, self_type_def_id: DefId) -> bool
pub fn find_trait_def_id(&self, trait_name: Spur) -> Option<DefId>
```

已有 DefId-keyed 的 trait 实现查询，可直接复用。

### 2.3 bounded_ty 的 Res

`HirWherePredicate.bounded_ty` 是 `HirTy`。其 `kind` 可以是：
- `HirTyKind::Path(_, path)` where `path.res` is:
  - `Res::Def(def_id, DefKind::Struct)` — 具体类型 ✓ 可检查
  - `Res::Def(def_id, DefKind::Enum)` — 具体类型 ✓ 可检查
  - `Res::Def(def_id, DefKind::Trait)` — 不合法（trait 不能作为 bounded type）
  - `Res::PrimTy(_)` — 原始类型（i32, bool 等）✓ 可检查（如果注册了 impl）
  - `Res::SelfTy(_)` — Self 类型（在 trait/impl 内）— 推迟（需要上下文）
  - `Res::Unknown` — 未解析类型 → 报错
  - `Res::Local(_)` — 不合法（局部变量不能作为 bounded type）
  - `Res::Err` — 已有错误，跳过

**关键限制**: 当 bounded type 是**类型参数**（如 `T` in `fn f<T>() where T: Clone`）时，`T` 在当前 resolver 中没有对应的 `DefId`（类型参数不是 top-level 定义）。这种情况下**无法检查**——因为 T 是抽象的，任何类型都可能代入。这正是 Rust 的行为：where clause 对类型参数是**声明性约束**，不是可检查的断言。

**因此，本阶段聚焦**: 当 bounded type 是**具体类型**（struct/enum/原始类型）时，验证 trait 实现。

## 3. 重构方案

### 3.1 新增 `WhereClauseError` 类型分类

在 `TypeError` 的 message 中添加前缀分类，便于测试和用户理解：

- `"where clause error: trait `{}` not found"` — Phase 1 已有
- `"where clause error: type `{}` does not implement trait `{}`"` — Phase 2 新增
- `"where clause error: bounded type `{}` is not a concrete type"` — 当 bounded_ty 是 Local/Lifetime 等不合法类型

### 3.2 扩展 `check_where_clause_for_generics`

```rust
fn check_where_clause_for_generics(
    generics: &HirGenerics,
    item_name: &str,
    resolver: &TraitResolver,  // 不再前缀 _
    interner: &Rodeo,
    errors: &mut Vec<TypeError>,
) {
    for pred in &generics.where_clause {
        // 解析 bounded_ty 的 DefId（如果是具体类型）
        let bounded_def_id = resolve_bounded_type_def_id(&pred.bounded_ty);

        for bound in &pred.bounds {
            if let HirTypeBound::Trait(tc) = bound {
                match tc.path.res {
                    Res::Def(trait_def_id, DefKind::Trait) => {
                        if let Some(type_def_id) = bounded_def_id {
                            // Phase 2: 具体类型 — 检查 trait 实现
                            if !resolver.implements_by_def_ids(trait_def_id, type_def_id) {
                                let type_name = format_hir_ty_name(&pred.bounded_ty, interner);
                                let trait_name = format_trait_name(tc, interner);
                                errors.push(TypeError::new(
                                    format!(
                                        "where clause error: type `{}` does not implement trait `{}` in {}",
                                        type_name, trait_name, item_name
                                    ),
                                    pred.span,
                                ));
                            }
                        }
                        // 如果 bounded_def_id 是 None（类型参数 T），跳过 — 不可检查
                    }
                    Res::Unknown | Res::Err => {
                        // Phase 1: trait not found
                        // ... (现有代码)
                    }
                    _ => {}
                }
            }
        }
    }
}
```

### 3.3 新增 `resolve_bounded_type_def_id` helper

```rust
/// Resolve the bounded type in a where clause to a DefId, if it's a concrete type.
///
/// Returns:
/// - `Some(def_id)` for concrete types (struct, enum)
/// - `None` for type parameters (T), Self, primitive types (deferred), or unresolvable types
fn resolve_bounded_type_def_id(bounded_ty: &HirTy) -> Option<DefId> {
    if let HirTyKind::Path(_, path) = &bounded_ty.kind {
        if let Res::Def(def_id, kind) = path.res {
            match kind {
                DefKind::Struct | DefKind::Enum => return Some(def_id),
                _ => {}
            }
        }
    }
    None
}
```

### 3.4 新增 `format_hir_ty_name` 和 `format_trait_name` helpers

用于生成用户友好的错误消息中的类型名和 trait 名。

## 4. J1-J6 检查

| # | 判据 | 满足情况 |
|---|------|----------|
| J1 | 架构设计对齐 | ✅ 与 04-type-system.md where clause 设计一致 |
| J2 | 单一职责 | ✅ `check_where_clause_for_generics` 只检查 where clause；`resolve_bounded_type_def_id` 只解析类型 |
| J3 | 单向流动 | ✅ check → resolve_bounded_type → implements_by_def_ids 单向 |
| J4 | 编译相关表达完整 | ✅ 覆盖具体类型；类型参数推迟（设计决策记录） |
| J5 | 阶段划分清晰 | ✅ 仍在 typeck/ 模块 |
| J6 | 科学合理粒度 | ✅ 新增 ~60 LOC + helper 函数 |

## 5. 测试计划 (§9.4.3 1:3+ ratio)

### 正向测试 (positive)
1. `where_clause_concrete_type_implements_trait` — `struct S; impl Foo for S { } fn f() where S: Foo { }` → 无错误
2. `where_clause_type_param_no_error` — `fn f<T>() where T: Clone { }` → 无错误（类型参数不可检查，不报错）

### 负向测试 (negative, ≥6)
1. `where_clause_concrete_type_does_not_implement_trait` — `struct S; fn f() where S: NonImplemented { }` → 报错
2. `where_clause_concrete_enum_does_not_implement` — enum 版本
3. `where_clause_multiple_bounds_one_unsatisfied` — `where S: Foo + Bar` where S: Foo but not Bar → 报错
4. `where_clause_struct_not_implement` — 另一个 struct 场景
5. `where_clause_trait_not_found_phase1_still_works` — Phase 1 回归（trait 不存在仍报错）
6. `where_clause_multiple_where_preds_one_fails` — 多个 where predicate，一个失败

比例: 2:6 = 1:3 ✓（刚好满足 1:3+）

## 6. 验收标准

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅
- 新增 8 测试全部通过 ✅
- Phase 1 测试全部回归通过 ✅
- worklog 记录 ✅

## 7. 设计决策记录

**推迟项** (B1 实现 < 设计):
- 类型参数 `T` 的 where clause 语义检查推迟——需要 trait bound 推断（Rust 的 trait solver），是 v0.5+ 范畴
- `Self` 类型作为 bounded type 推迟——需要 trait/impl 上下文
- 原始类型（i32, bool）作为 bounded type 推迟——需要注册原始类型的 trait impl

这些推迟是合理的——Rust 本身也不在编译时检查类型参数的 where clause（它们是声明性约束，在 monomorphization 时才验证）。本阶段聚焦具体类型检查，这是可立即实现的语义增强。

## 8. 结论

定稿 — scope 清晰，1 轮自审无 P0/P1 缺陷。实现 ~60 LOC + 8 测试。
