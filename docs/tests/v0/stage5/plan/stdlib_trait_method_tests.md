# Test Plan: Stage 5.36 — Stdlib Trait Method Signatures

> **Stage**: 5.36
> **Version**: v0.11.31 → v0.11.32
> **Test file**: `tests/v0/stage5/plan/stdlib_trait_method_tests.rs`
> **Test count**: 24 new tests (1106 → 1130 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `StdlibTraitMethod` + `StdlibSelfKind` + `stdlib_trait_methods()` +
`stdlib_trait_method_count()` + `find_stdlib_trait_method()` +
`is_stdlib_trait_method()` + `stdlib_traits_with_method()` 的正确性。

## 2. 覆盖场景

### 2.1 正向查询（slice 返回）

- Clone → 2 methods (clone, clone_from)
- Drop → 1 method (drop, SelfByMutRef, returns Unit)
- Default → 1 method (default, NoSelf — associated function)
- Display → 1 method (fmt, SelfByRef, param_count=1)
- PartialEq → 2 methods (eq, ne) — both SelfByRef, return Bool
- Ord → 1 method (cmp, SelfByRef)
- Iterator → 1 method (next, SelfByMutRef, returns StdType=Option)

### 2.2 边界 / markers

- Copy/Send/Sync/Sized/Unpin/Eq → `Some(&[])` (空但非 None)
- 测试 `stdlib_trait_methods("BogusTrait") == None`
- 测试 `stdlib_trait_methods("From") == None` (From 未注册)
- 测试 `stdlib_trait_methods("") == None` (空字符串)

### 2.3 算术 op 精确性

- Add/Sub/Mul/Div/Rem/BitAnd/BitOr/BitXor/Shl/Shr — 每个有独立 const 表，方法名正确
- AddAssign/SubAssign/.../ShrAssign — 每个有独立 const 表，返回 Unit, SelfByMutRef
- Add::sub 应为 None（不同 op 名）
- Add::add_assign 应为 None（不同 trait）

### 2.4 method_count

- Clone → Some(2)
- Drop → Some(1)
- Copy → Some(0)（marker）
- BogusTrait → None

### 2.5 find_stdlib_trait_method

- 命中: (Clone, clone), (Iterator, next)
- 未命中: (Clone, bogus), (BogusTrait, clone), (Clone, next)
- 算术 op 精确匹配: (Add, add) hit, (Sub, sub) hit, (Add, sub) miss

### 2.6 is_stdlib_trait_method

- true: (Clone, clone), (Clone, clone_from), (Iterator, next),
  (Default, default), (Add, add)
- false: (Clone, next), (Iterator, clone), (Bogus, clone), (Copy, clone) (marker)

### 2.7 stdlib_traits_with_method (反向查询)

- "clone" → 包含 Clone
- "fmt" → 包含 Display + Debug
- "bogus_method" → 空 Vec

### 2.8 StdlibTraitMethod 辅助

- `has_self()` 对 NoSelf 返回 false，对其他三种 self_kind 返回 true
- `StdlibTraitMethod` 派生 PartialEq/Eq — 直接比较

## 3. 测试统计

- 新增: 24 tests
- 基线: 1106 tests
- 总计: 1130 tests
- 2 ignored (pre-existing,未影响)

## 4. 依赖

- 上游: Stage 5.35 (`StdlibTypeKind` + `resolve_stdlib_type()`)
- 下游: Stage 5.37+ (dyn Trait MIR lowering) — 将使用 `stdlib_trait_methods()`
  查询 vtable 函数签名

## 5. CI/CD 验证

```
cargo clean: clean
cargo test: 1130 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
