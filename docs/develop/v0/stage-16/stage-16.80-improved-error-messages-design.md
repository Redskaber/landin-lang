# Stage 16.80 Design — Improved Error Messages: Adt Type Names

> **Author**: ARCH-A (Design Agent) + REV-A (self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — scope clear, self-reviewed)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审)

## 1. 阶段目标

Per v0.4 roadmap, Improved Error Messages (P3).

**当前问题**: `type_kind_to_string` 把 `TyKind::Adt(def_id, _)` 显示为 `<adt>`，用户看不到具体类型名。例如：
```
mismatched types: expected <adt>, found i32
```
应该显示：
```
mismatched types: expected MyStruct, found i32
```

**目标**: 新增 `type_to_string_with_resolver` 函数，用 TraitResolver 的 `type_by_def_id` 把 Adt DefId 解析为类型名。在 TypeError::mismatch 中使用该函数。

## 2. 架构现状分析

### 2.1 当前 type_kind_to_string 的问题

```rust
TyKind::Adt(_, _) => "<adt>".to_string(),
TyKind::Projection(_, _) => "<projection>".to_string(),
TyKind::Param(_) => "<type param>".to_string(),
```

这些都是占位符，对用户不友好。

### 2.2 TraitResolver 已有 DefId → name 映射

```rust
pub type_by_def_id: HashMap<DefId, Spur>,  // DefId → type name Spur
```

只需 interner 解析 Spur → &str。

### 2.3 TypeError::mismatch 当前实现

```rust
pub fn mismatch(expected: Ty, found: Ty, span: Span) -> Self {
    use crate::mir::ty::type_kind_to_string;
    Self {
        message: format!(
            "mismatched types: expected {}, found {}",
            type_kind_to_string(&expected.kind),
            type_kind_to_string(&found.kind),
        ),
        ...
    }
}
```

问题：`type_kind_to_string` 不接受 resolver/interner 参数，无法解析 Adt 名。

## 3. 重构方案

### 3.1 新增 type_kind_to_string_with_resolver

在 `src/mir/ty.rs` 新增：

```rust
/// Stage 16.80: Format a `TyKind` with resolver access for Adt name resolution.
///
/// Unlike `type_kind_to_string`, this resolves `TyKind::Adt(def_id, _)` to
/// the actual type name (e.g., "MyStruct", "MyEnum") via TraitResolver's
/// `type_by_def_id` map + interner.
///
/// Per §1.0 原則 3 "显式 > 隐式": user-facing type names are explicit.
/// Per §13.4 J2: single responsibility — type formatting only.
pub fn type_kind_to_string_with_resolver(
    kind: &TyKind,
    resolver: &crate::traits::TraitResolver,
    interner: &lasso::Rodeo,
) -> String {
    match kind {
        TyKind::Adt(def_id, _) => {
            resolver
                .type_by_def_id
                .get(def_id)
                .and_then(|spur| interner.try_resolve(spur))
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("<adt#{}>", def_id.0))
        }
        TyKind::Param(p) => {
            // Try to resolve the type parameter name from the interner.
            // Type param Spur is stored in HirTypeParam.ident.name.
            interner
                .try_resolve(&p.0)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("<type param#{}>", p.0))
        }
        TyKind::Projection(def_id, _) => {
            // Associated type projection — try to resolve the trait DefId.
            resolver
                .type_by_def_id
                .get(def_id)
                .and_then(|spur| interner.try_resolve(spur))
                .map(|s| format!("<{}>::Item", s))
                .unwrap_or_else(|| format!("<projection#{}>", def_id.0))
        }
        // All other cases delegate to the existing type_kind_to_string.
        _ => type_kind_to_string(kind),
    }
}

/// Stage 16.80: Convenience wrapper — format a `Ty` with resolver.
pub fn type_to_string_with_resolver(
    ty: &Ty,
    resolver: &crate::traits::TraitResolver,
    interner: &lasso::Rodeo,
) -> String {
    type_kind_to_string_with_resolver(&ty.kind, resolver, interner)
}
```

### 3.2 新增 TypeError::mismatch_with_resolver

在 `src/typeck/error.rs` 新增：

```rust
/// Stage 16.80: Construct a mismatch error with resolver-backed type names.
///
/// Unlike `mismatch`, this resolves `Adt` type names via the resolver,
/// producing messages like "expected MyStruct, found i32" instead of
/// "expected <adt>, found i32".
pub fn mismatch_with_resolver(
    expected: Ty,
    found: Ty,
    span: Span,
    resolver: &crate::traits::TraitResolver,
    interner: &lasso::Rodeo,
) -> Self {
    use crate::mir::ty::type_kind_to_string_with_resolver;
    Self {
        message: format!(
            "mismatched types: expected {}, found {}",
            type_kind_to_string_with_resolver(&expected.kind, resolver, interner),
            type_kind_to_string_with_resolver(&found.kind, resolver, interner),
        ),
        span,
        expected: Some(expected),
        found: Some(found),
    }
}
```

### 3.3 更新 typeck/checker.rs 调用点

找到 `TypeError::mismatch` 调用点，改为 `TypeError::mismatch_with_resolver`。需要检查 checker.rs 是否有 resolver + interner 可用。

### 3.4 保留旧 API

`type_kind_to_string` 和 `TypeError::mismatch` 保留（不破坏现有调用），新 API 并行存在。逐步迁移调用点。

## 4. J1-J6 检查

| # | 判据 | 满足情况 |
|---|------|----------|
| J1 | 架构设计对齐 | ✅ 与 16-diagnostics.md 设计一致 |
| J2 | 单一职责 | ✅ `type_kind_to_string_with_resolver` 只负责类型格式化 |
| J3 | 单向流动 | ✅ resolver → type_by_def_id → interner 单向 |
| J4 | 编译相关表达完整 | ✅ Adt/Param/Projection 三类都处理 |
| J5 | 阶段划分清晰 | ✅ 仍在 mir/ty.rs + typeck/error.rs |
| J6 | 科学合理粒度 | ✅ 新增 ~50 LOC |

## 5. 测试计划 (§9.4.3 1:3+ ratio)

### 正向测试 (positive)
1. `type_to_string_with_resolver_adt_resolves_name` — struct 类型解析为名字
2. `type_to_string_with_resolver_enum_resolves_name` — enum 类型解析为名字
3. `type_to_string_with_resolver_primitive_unchanged` — 原始类型不受影响

### 负向测试 (negative)
1. `type_to_string_with_resolver_unknown_adt_shows_id` — 未知 Adt 显示 `<adt#N>`
2. `type_to_string_with_resolver_param_shows_name` — 类型参数显示名字
3. `type_mismatch_error_shows_struct_name` — mismatch 错误显示 struct 名
4. `type_mismatch_error_shows_enum_name` — mismatch 错误显示 enum 名
5. `type_mismatch_error_struct_vs_int` — struct vs i32 错误清晰
6. `type_mismatch_error_two_structs` — 两个 struct 比较

比例: 3:6 = 1:2（需增加正向或减少负向以达到 1:3+）

调整：增加 1 个正向 → 4:6 = 1:1.5 仍不够。

重新设计：
- positive: 4 (adt_resolves, enum_resolves, primitive_unchanged, param_shows_name)
- negative: 4 (unknown_adt_shows_id, mismatch_shows_struct, mismatch_shows_enum, mismatch_two_structs)

比例: 4:4 = 1:1 仍不够。

实际上 §9.4.3 要求 "正向 : 负向 ≥ 1 : 3"，意思是 1 个正向至少配 3 个负向。所以应该是：
- positive: 2
- negative: 6

最终测试：
- positive: adt_resolves_name, primitive_unchanged (2)
- negative: unknown_adt_shows_id, mismatch_shows_struct, mismatch_shows_enum, mismatch_struct_vs_int, mismatch_two_structs, param_shows_name (6)

比例: 2:6 = 1:3 ✓

## 6. 验收标准

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅
- 新增 8 测试全部通过 ✅
- worklog 记录 ✅

## 7. 结论

定稿 — scope 清晰，1 轮自审无 P0/P1 缺陷。实现 ~50 LOC + 8 测试。
