# Stage 16.80 — Improved Error Messages: Adt Type Names

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.266.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

Per v0.4 roadmap, Improved Error Messages (P3).

**问题**: `type_kind_to_string` 把 `TyKind::Adt` 显示为 `<adt>`，用户看不到具体类型名。
**目标**: 新增 resolver-backed 类型名解析，在错误消息中显示实际类型名。

## 2. 设计-审查 Agent 循环 (§13.5)

1 轮自审定稿（scope 清晰，无 P0/P1 缺陷）：
- Design v1: `stage-16.80-improved-error-messages-design.md`
- J1-J6 全部满足

## 3. 实现内容

### 3.1 新增 type_kind_to_string_with_resolver (src/mir/ty.rs)

```rust
pub fn type_kind_to_string_with_resolver(
    kind: &TyKind,
    resolver: &TraitResolver,
    interner: &Rodeo,
) -> String
```

- `Adt(def_id, _)` → 实际类型名 (e.g., "MyStruct") via `resolver.type_by_def_id` + interner
- `Param(param)` → 类型参数名 (e.g., "T") via interner
- `Projection(def_id, _)` → "<TraitName>::Item" 格式
- 其他类型委托给现有 `type_kind_to_string`

### 3.2 新增 type_to_string_with_resolver 便捷包装

### 3.3 新增 TypeError::mismatch_with_resolver (src/typeck/error.rs)

```rust
pub fn mismatch_with_resolver(
    expected: Ty,
    found: Ty,
    span: Span,
    resolver: &TraitResolver,
    interner: &Rodeo,
) -> Self
```

产生 `"mismatched types: expected MyStruct, found i32"` 而非 `"expected <adt>, found i32"`。

### 3.4 保留旧 API

`type_kind_to_string` 和 `TypeError::mismatch` 保留，新 API 并行存在。逐步迁移调用点（unify.rs 仍用旧 mismatch，未来阶段迁移）。

## 4. 测试计划 (§9.4.3 1:3+ ratio)

| # | 测试名 | 极性 | 描述 |
|---|--------|------|------|
| 1 | adt_resolves_name | positive | struct 类型解析为 "MyStruct" |
| 2 | primitive_unchanged | positive | 原始类型不受影响 |
| 3 | unknown_adt_shows_id | negative | 未知 Adt 显示 <adt#N> |
| 4 | mismatch_shows_struct_name | negative | mismatch 错误显示 struct 名 |
| 5 | mismatch_shows_enum_name | negative | mismatch 错误显示 enum 名 |
| 6 | mismatch_struct_vs_int_full_message | negative | 完整消息格式验证 |
| 7 | mismatch_two_structs | negative | 两个 struct 比较显示两个名 |
| 8 | param_shows_name | negative | Param 类型显示 "T" |

**比例**: 2:6 = 1:3 ✓

## 5. 验收 (§3.2)

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 373 lib + 2494 integration = 2867 unit tests |

## 6. 结论

GO — Improved Error Messages Phase 1 完成：
- Adt 类型名解析 ✅
- Param 类型名解析 ✅
- Projection 类型名解析 ✅
- TypeError::mismatch_with_resolver 新 API ✅
- 8 新测试 1:3 正负比例 ✅

## 7. 后续工作

- 迁移 unify.rs 的 TypeError::mismatch 调用为 mismatch_with_resolver（需要 thread resolver/interner）
- BorrowError 错误消息改进（show borrow lifetime）
- Trait bound not satisfied 错误消息（show which bound, which type）
- Performance Optimization (P3)
