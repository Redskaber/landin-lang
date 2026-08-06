# Stage 16.85 Design — Migrate expr_operand.rs Type Errors to Use Resolver

> **Author**: ARCH-A (Design Agent) + REV-A (self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — scope clear, self-reviewed)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审)

## 1. 阶段目标

`expr_operand.rs:2459` 的 "no method found" 错误用 `type_kind_to_string`，显示 `<adt>`。本阶段让 MIR lower 也能显示实际类型名。

## 2. 架构现状分析

### 2.1 MirLowerCtxt 无 resolver

```rust
pub struct MirLowerCtxt<'a> {
    pub interner: &'a Rodeo,
    pub unify: UnificationTable,
    // ... no resolver
}
```

### 2.2 时序问题

MIR lower 在 typeck 之前运行，所以 unify 的 resolver 此时未设置。需要在 MIR lower 开始前就设置 resolver。

### 2.3 调用链

```
driver.rs lower_hir_body_to_mir_full_with_dyn_trait_plan(body, interner, hir, return_ty, plan)
  → MirLowerCtxt::new(interner, span)
  → cx.lower_body() → expr_operand.rs "no method found" error
```

## 3. 重构方案

### 3.1 MirLowerCtxt 新增 resolver 字段

```rust
pub struct MirLowerCtxt<'a> {
    pub interner: &'a Rodeo,
    pub unify: UnificationTable,
    /// Stage 16.85: Optional resolver for rich error messages.
    resolver: Option<&'a crate::traits::TraitResolver>,
    // ...
}
```

### 3.2 新增 set_resolver + format_ty

```rust
pub fn set_resolver(&mut self, resolver: &'a crate::traits::TraitResolver) {
    self.resolver = Some(resolver);
}

fn format_ty(&self, ty: &Ty) -> String {
    if let Some(resolver) = self.resolver {
        crate::mir::ty::type_to_string_with_resolver(ty, resolver, self.interner)
    } else {
        crate::mir::ty::type_to_string(ty)
    }
}
```

### 3.3 更新 lower_hir_body_to_mir_full_with_dyn_trait_plan

新增 `resolver: Option<&TraitResolver>` 参数，在创建 cx 后调用 `cx.set_resolver`。

### 3.4 替换 expr_operand.rs 的 type_kind_to_string

用 `cx.format_ty(&recv_ty)` 替代。

### 3.5 更新 driver.rs 调用点

传入 `Some(&trait_resolver)`。

## 4. J1-J6 检查

| # | 判据 | 满足情况 |
|---|------|----------|
| J1 | 架构设计对齐 | ✅ 与 16-diagnostics.md 一致 |
| J2 | 单一职责 | ✅ `format_ty` 只负责类型格式化 |
| J3 | 单向流动 | ✅ driver → set_resolver → format_ty |
| J4 | 编译相关表达完整 | ✅ expr_operand.rs 1 处迁移 |
| J5 | 阶段划分清晰 | ✅ 仍在 mir/lower/ |
| J6 | 科学合理粒度 | ✅ ~30 LOC 新增 |

## 5. 测试计划 (§9.4.3 1:3+ ratio)

### 正向测试 (positive)
1. `mir_lower_format_ty_with_resolver_shows_name` — format_ty 显示 "MyStruct"
2. `mir_lower_format_ty_without_resolver_falls_back` — fallback 正常

### 负向测试 (negative)
1. `compile_no_method_found_shows_struct_name` — "no method found for type MyStruct"
2. `compile_method_on_struct_shows_name` — method call on struct 显示名
3. `compile_method_on_enum_shows_name` — method call on enum 显示名
4. `compile_method_on_unknown_type_shows_id` — unknown type 显示 ID
5. `compile_method_on_primitive_unchanged` — primitive 类型不受影响
6. `compile_method_on_ref_struct_shows_name` — &MyStruct 显示名

比例: 2:6 = 1:3 ✓

## 6. 验收标准

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅
- 新增 8 测试全部通过 ✅

## 7. 结论

定稿 — scope 清晰，1 轮自审无 P0/P1 缺陷。实现 ~30 LOC + 8 测试。
