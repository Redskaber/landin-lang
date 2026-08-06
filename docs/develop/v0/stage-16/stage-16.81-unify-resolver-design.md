# Stage 16.81 Design — Migrate unify.rs to mismatch_with_resolver

> **Author**: ARCH-A (Design Agent) + REV-A (self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — scope clear, self-reviewed)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审)

## 1. 阶段目标

Stage 16.80 添加了 `TypeError::mismatch_with_resolver` 但 unify.rs 仍用旧 `TypeError::mismatch`，导致实际类型错误仍显示 `<adt>` 而非类型名。

**目标**: 让 unify.rs 在产生 mismatch 错误时使用 resolver-backed 类型名，使实际编译错误显示 "expected MyStruct, found i32"。

## 2. 架构现状分析

### 2.1 当前 unify.rs 结构

```rust
pub struct UnificationTable {
    // ... no resolver/interner
}

impl UnificationTable {
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), Box<TypeError>> {
        let a = self.resolve(a);
        let b = self.resolve(b);
        self.unify_resolved(&a, &b)  // calls TypeError::mismatch (old API)
    }
}
```

10 处 `TypeError::mismatch` 调用，全部在 `unify_resolved` 内。

### 2.2 调用链

```
driver.rs typeck_main_body()
  → TypeChecker::with_unify(shared_unify)
  → tc.check_mir_body_with_tables(mir, field_ty_table)
    → self.unify.unify(&a, &b)  // 15 处 in checker.rs
      → unify_resolved → TypeError::mismatch (old, no resolver)
```

### 2.3 问题

`UnificationTable` 不持有 resolver/interner 引用，无法在 mismatch 时解析 Adt 名。

## 3. 重构方案

### 3.1 在 UnificationTable 添加可选 resolver/interner

```rust
pub struct UnificationTable {
    // ... existing fields ...

    /// Stage 16.81: Optional resolver for rich error messages.
    /// When set, mismatch errors use `mismatch_with_resolver` to show
    /// actual type names (e.g., "MyStruct") instead of placeholders ("<adt>").
    /// None = use legacy `mismatch` (for tests/standalone usage).
    resolver: Option<*const crate::traits::TraitResolver>,
    interner: Option<*const lasso::Rodeo>,
}
```

**为什么用裸指针**：`UnificationTable` 需要 `&mut self` 进行 unify（绑定 InferVar），但同时需要 `&TraitResolver` 和 `&Rodeo`（只读）。Rust 借用规则不允许 `&mut self` + `&resolver` 同时存在（即使 resolver 在不同字段）。使用 `*const` 裸指针绕过借用检查，安全性由调用者保证（resolver/intner 在 typeck 期间不变）。

**替代方案**：泛型参数 `UnificationTable<'a>` — 会传染到所有调用点（15+ 处），违反 §13.4 J6（粒度合理）。裸指针方案更内聚。

### 3.2 新增 set_resolver 方法

```rust
impl UnificationTable {
    /// Stage 16.81: Set the resolver/interner for rich error messages.
    ///
    /// After calling this, `unify` will use `mismatch_with_resolver` to
    /// produce errors with actual type names. The references must outlive
    /// the UnificationTable.
    ///
    /// Per §23: `set_resolver` follows `<verb>_<noun>` pattern.
    pub fn set_resolver(
        &mut self,
        resolver: &crate::traits::TraitResolver,
        interner: &lasso::Rodeo,
    ) {
        self.resolver = Some(resolver as *const _);
        self.interner = Some(interner as *const _);
    }
}
```

### 3.3 新增私有 helper：make_mismatch

```rust
/// Stage 16.81: Construct a mismatch error, using resolver if available.
fn make_mismatch(&self, expected: Ty, found: Ty, span: Span) -> TypeError {
    if let (Some(resolver_ptr), Some(interner_ptr)) = (self.resolver, self.interner) {
        // SAFETY: resolver/interner are set once before typeck and remain
        // valid for the lifetime of the UnificationTable.
        let resolver = unsafe { &*resolver_ptr };
        let interner = unsafe { &*interner_ptr };
        TypeError::mismatch_with_resolver(expected, found, span, resolver, interner)
    } else {
        TypeError::mismatch(expected, found, span)
    }
}
```

### 3.4 替换 10 处 TypeError::mismatch 为 self.make_mismatch

### 3.5 在 driver.rs typeck_main_body 设置 resolver

```rust
fn typeck_main_body(
    mir: &mut MirBody,
    shared_unify: &mut UnificationTable,
    fn_sig_table: &FnSigTable,
    field_ty_table: &FieldTyTable,
    resolver: &TraitResolver,  // 新增参数
    interner: &Rodeo,          // 新增参数
) -> ... {
    let mut tc = TypeChecker::with_unify(std::mem::take(shared_unify));
    tc.fn_sigs = fn_sig_table.sigs.clone();
    tc.unify.set_resolver(resolver, interner);  // 新增
    tc.check_mir_body_with_tables(mir, Some(field_ty_table));
    ...
}
```

## 4. J1-J6 检查

| # | 判据 | 满足情况 |
|---|------|----------|
| J1 | 架构设计对齐 | ✅ 与 16-diagnostics.md 一致 |
| J2 | 单一职责 | ✅ `make_mismatch` 只负责错误构造 |
| J3 | 单向流动 | ✅ driver → set_resolver → unify → make_mismatch |
| J4 | 编译相关表达完整 | ✅ 10 处 mismatch 全部迁移 |
| J5 | 阶段划分清晰 | ✅ 仍在 typeck/ |
| J6 | 科学合理粒度 | ✅ ~40 LOC 新增 + 10 处替换 |

## 5. SAFETY 审查

**裸指针安全性论证**：
1. `set_resolver` 只在 `typeck_main_body` 开头调用一次，之后 resolver/interner 引用不变
2. resolver/interner 的生命周期 ≥ UnificationTable（由 driver 保证）
3. `make_mismatch` 只读访问 resolver/interner（通过 `&*ptr`）
4. 没有多线程（单线程 typeck）

**替代方案**：如果裸指针引起审查争议，可改为 `Option<&'a TraitResolver>` + lifetime parameter，但这会传染到所有 UnificationTable 使用点。本设计选择内聚方案。

## 6. 测试计划 (§9.4.3 1:3+ ratio)

### 正向测试 (positive)
1. `unify_with_resolver_shows_struct_name` — struct mismatch 显示实际名
2. `unify_without_resolver_falls_back` — 无 resolver 时用旧 API

### 负向测试 (negative)
1. `compile_mismatch_struct_int_shows_name` — 编译时错误含 "MyStruct"
2. `compile_mismatch_two_structs_shows_names` — 两个 struct 名都显示
3. `compile_mismatch_enum_int_shows_name` — enum 名显示
4. `compile_mismatch_struct_ref_shows_name` — &MyStruct 名显示
5. `compile_mismatch_tuple_struct_shows_name` — tuple struct 名显示
6. `compile_mismatch_array_shows_name` — 数组类型显示

比例: 2:6 = 1:3 ✓

## 7. 验收标准

- `cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅
- 新增 8 测试全部通过 ✅
- worklog 记录 ✅

## 8. 结论

定稿 — scope 清晰，1 轮自审无 P0/P1 缺陷。实现 ~40 LOC + 10 处替换 + 8 测试。
