# Stage 16.82 Design — BorrowError Message Improvements

> **Author**: ARCH-A (Design Agent) + REV-A (self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — scope clear, self-reviewed)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审)

## 1. 阶段目标

Stage 16.80-16.81 改进了 TypeError 消息（显示实际类型名）。本阶段对 BorrowError 做类似改进。

**当前问题**: BorrowError 消息不显示类型名。例如：
- `"cannot borrow moved value"` — 不显示什么类型的值被 move
- `"cannot assign twice to immutable variable"` — 不显示变量名
- `"lifetime error: type {} does not outlive region {}"` — 用 `type_kind_to_string` 显示 `<adt>`

**目标**: 在 BorrowChecker 有 resolver/interner 时，使用 `type_to_string_with_resolver` 显示实际类型名 + 变量名。

## 2. 架构现状分析

### 2.1 BorrowChecker 已有 resolver/interner

```rust
pub struct BorrowChecker<'a> {
    // ...
    resolver: Option<&'a TraitResolver>,
    interner: Option<&'a Rodeo>,
}
```

已有 `with_resolver` 构造函数，driver 已调用。无需额外 threading。

### 2.2 BorrowError 构造点

| 位置 | 当前消息 | 改进方向 |
|------|---------|---------|
| mod.rs L278 | `"lifetime error: type {} does not outlive region {}"` | 用 resolver 类型名 |
| mod.rs L667 | `"cannot borrow moved value"` | 加变量名 |
| mod.rs L754 | `"use of moved value"` | 加变量名 |
| mod.rs L790 | `"use of moved value"` | 加变量名 |
| mod.rs L795 | `"cannot borrow moved value"` | 加变量名 |
| mod.rs L904 | `"cannot assign twice to immutable variable"` | 加变量名 |
| borrow_set.rs L96 | borrow conflict | 加变量名 |

### 2.3 Place 到变量名的映射

MIR `Place` 有 `PlaceKind::Local(LocalId)`。LocalId 可通过 `mir.local_decls[id]` 获取 `LocalDecl`，但 LocalDecl 没有名字字段。变量名在 HIR 层。

**简化方案**: 用 LocalId 作为标识（如 `local#3`），不解析实际变量名。这比 "moved value" 更有用，且不需要 HIR 访问。

**更优方案**: 如果 resolver 有 local_names 映射，用实际名。但 TraitResolver 不存 local names。推迟到未来阶段。

## 3. 重构方案

### 3.1 新增 BorrowChecker helper：format_ty

```rust
impl<'a> BorrowChecker<'a> {
    /// Stage 16.82: Format a Ty with resolver if available, else fallback.
    fn format_ty(&self, ty: &Ty) -> String {
        if let (Some(resolver), Some(interner)) = (self.resolver, self.interner) {
            crate::mir::ty::type_to_string_with_resolver(ty, resolver, interner)
        } else {
            crate::mir::ty::type_to_string(ty)
        }
    }

    /// Stage 16.82: Format a Place for error messages.
    /// Returns "local#N" (simplified — actual var names need HIR access, deferred).
    fn format_place(&self, mir: &MirBody, place: &Place) -> String {
        match &place.kind {
            PlaceKind::Local(id) => format!("local#{}", id.0),
            PlaceKind::Projection(base, _) => {
                format!("{:?}", base)  // simplified
            }
        }
    }
}
```

### 3.2 改进 lifetime error 消息

**前**: `"lifetime error: type {} does not outlive region {}"` with `type_kind_to_string`
**后**: `"lifetime error: type {} does not outlive region {}"` with `format_ty`

### 3.3 改进 moved value 消息

**前**: `"cannot borrow moved value"`
**后**: `"cannot borrow moved value: {}"` with place info

### 3.4 改进 immutable assign 消息

**前**: `"cannot assign twice to immutable variable"`
**后**: `"cannot assign twice to immutable variable: local#N"`

## 4. J1-J6 检查

| # | 判据 | 满足情况 |
|---|------|----------|
| J1 | 架构设计对齐 | ✅ 与 16-diagnostics.md 一致 |
| J2 | 单一职责 | ✅ `format_ty` 只负责类型格式化 |
| J3 | 单向流动 | ✅ BorrowChecker → format_ty → type_to_string_with_resolver |
| J4 | 编译相关表达完整 | ✅ 所有 BorrowError 消息改进 |
| J5 | 阶段划分清晰 | ✅ 仍在 borrowck/ |
| J6 | 科学合理粒度 | ✅ ~40 LOC 新增 |

## 5. 测试计划 (§9.4.3 1:3+ ratio)

### 正向测试 (positive)
1. `borrow_error_lifetime_shows_type_name` — lifetime error 显示实际类型名
2. `borrow_error_no_resolver_falls_back` — 无 resolver 时 fallback 正常

### 负向测试 (negative)
1. `compile_move_after_borrow_shows_place` — move after borrow 报错含 place
2. `compile_assign_immutable_shows_local` — 不可变重赋值报错含 local#
3. `compile_use_after_move_shows_place` — use after move 报错含 place
4. `compile_borrow_conflict_shows_place` — borrow conflict 报错含 place
5. `compile_double_mut_borrow_shows_place` — 双重 &mut 报错含 place
6. `compile_move_in_borrowed_shows_place` — move borrowed 报错含 place

比例: 2:6 = 1:3 ✓

## 6. 验收标准

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅
- 新增 8 测试全部通过 ✅

## 7. 结论

定稿 — scope 清晰，1 轮自审无 P0/P1 缺陷。实现 ~40 LOC + 8 测试。
