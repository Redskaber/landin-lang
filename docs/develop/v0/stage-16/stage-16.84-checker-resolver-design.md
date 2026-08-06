# Stage 16.84 Design — Migrate checker.rs Type Errors to Use Resolver

> **Author**: ARCH-A (Design Agent) + REV-A (self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — scope clear, self-reviewed)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审)

## 1. 阶段目标

checker.rs 中 6 处 `type_kind_to_string` 调用产生 "expected function, found <adt>" 错误。本阶段让这些错误也显示实际类型名。

## 2. 架构现状分析

### 2.1 TypeChecker 无 resolver/interner

```rust
pub struct TypeChecker {
    pub unify: UnificationTable,  // Stage 16.81: unify 有 resolver/interner
    pub errors: Vec<TypeError>,
    // ... no resolver/interner fields
}
```

但 `unify` 通过 Stage 16.81 的 `set_resolver` 持有 resolver/interner 引用。

### 2.2 checker.rs 的 6 处 type_kind_to_string

| 行 | 错误消息 |
|----|---------|
| L479 | "expected function, found {}" (Call func) |
| L698 | "expected function, found {}" (Call func) |
| L743 | "switch discriminant must be integer or bool, found {}" |
| L771 | "if condition must be bool, found {}" |
| L883 | "match arm type mismatch: expected {}, found {}" |
| L899 | "match arm type mismatch: expected {}, found {}" |

### 2.3 UnificationTable 的 resolver 字段

```rust
resolver: Option<*const TraitResolver>,
interner: Option<*const Rodeo>,
```

当前只有 `set_resolver`，没有 getter。

## 3. 重构方案

### 3.1 新增 UnificationTable getter

```rust
/// Stage 16.84: Get the resolver reference (if set).
pub fn resolver(&self) -> Option<&TraitResolver> {
    self.resolver.map(|ptr| unsafe { &*ptr })
}

/// Stage 16.84: Get the interner reference (if set).
pub fn interner(&self) -> Option<&Rodeo> {
    self.interner.map(|ptr| unsafe { &*ptr })
}
```

### 3.2 新增 TypeChecker::format_ty

```rust
/// Stage 16.84: Format a Ty with resolver if available (via unify).
fn format_ty(&self, ty: &Ty) -> String {
    if let (Some(resolver), Some(interner)) = (self.unify.resolver(), self.unify.interner()) {
        crate::mir::ty::type_to_string_with_resolver(ty, resolver, interner)
    } else {
        crate::mir::ty::type_to_string(ty)
    }
}
```

### 3.3 替换 6 处 type_kind_to_string 为 self.format_ty

## 4. J1-J6 检查

| # | 判据 | 满足情况 |
|---|------|----------|
| J1 | 架构设计对齐 | ✅ 与 16-diagnostics.md 一致 |
| J2 | 单一职责 | ✅ `format_ty` 只负责类型格式化 |
| J3 | 单向流动 | ✅ TypeChecker → unify.resolver() → type_to_string_with_resolver |
| J4 | 编译相关表达完整 | ✅ 6 处全部迁移 |
| J5 | 阶段划分清晰 | ✅ 仍在 typeck/ |
| J6 | 科学合理粒度 | ✅ ~30 LOC 新增 |

## 5. 测试计划 (§9.4.3 1:3+ ratio)

### 正向测试 (positive)
1. `checker_format_ty_with_resolver_shows_name` — format_ty 显示 "MyStruct"
2. `checker_format_ty_without_resolver_falls_back` — fallback 正常

### 负向测试 (negative)
1. `compile_expected_function_found_struct_shows_name` — "found MyStruct"
2. `compile_if_condition_must_be_bool_shows_name` — "found MyStruct"
3. `compile_switch_discriminant_shows_name` — "found MyStruct"
4. `compile_match_arm_mismatch_shows_name` — 含类型名
5. `compile_call_non_function_shows_name` — "found MyStruct"
6. `compile_method_call_non_function_shows_name` — "found MyStruct"

比例: 2:6 = 1:3 ✓

## 6. 验收标准

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅
- 新增 8 测试全部通过 ✅

## 7. 结论

定稿 — scope 清晰，1 轮自审无 P0/P1 缺陷。实现 ~30 LOC + 8 测试。
