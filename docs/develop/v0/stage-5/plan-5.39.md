# Stage 5.39 开发计划：stdlib vtable construction planner

> **阶段**: Stage 5.39
> **版本**: v0.11.34 → v0.11.35
> **状态**: ✅ Complete

## 1. 目标

在 Stage 5.36-5.38（trait 方法签名 / slot 布局 / 字节大小）基础上，添加
**vtable 构造计划器**：给定一个 trait 名 + impl 提供的方法名集合，生成一个
有序的 vtable plan，每个 slot 标注：
- slot_index（来自 Stage 5.37）
- method_name（trait 声明的方法名）
- provided（impl 是否提供了该方法）

这样 codegen 在 emit `@.vtable.<trait>.<type>` 全局时只需遍历 plan，对
`provided=true` 的 slot 填入 impl 方法的 LLVM symbol，对 `provided=false`
的 slot 填入 null（或 panic stub），无需在 codegen 内重复推导 slot 顺序。

这是 dyn Trait codegen 的"最后一公里"静态规划——把 trait 声明顺序、impl
覆盖情况、slot 索引一次性合并为可直接消费的 plan。

## 2. 设计

### 2.1 新增类型

```rust
/// 单个 vtable plan entry：slot 索引 + 方法名 + impl 是否提供。
pub struct StdlibVtablePlanEntry {
    pub slot_index: u32,
    pub method_name: &'static str,
    pub provided: bool,
}

/// 完整 vtable 构造计划：trait 名 + 有序 entry 列表。
pub struct StdlibVtablePlan {
    pub trait_name: &'static str,
    pub entries: Vec<StdlibVtablePlanEntry>,
}
```

### 2.2 新增 API

| API | 签名 | 用途 |
|-----|------|------|
| `stdlib_vtable_plan` | `(trait_name, provided_method_names: &[&str]) -> Option<StdlibVtablePlan>` | 生成完整 vtable 计划 |
| `stdlib_vtable_plan_entry_count` | `(trait_name) -> Option<u32>` | plan entry 总数（= slot_count） |
| `stdlib_vtable_plan_is_complete` | `(&StdlibVtablePlan) -> bool` | 所有 slot 都 provided |
| `stdlib_vtable_plan_missing_methods` | `(&StdlibVtablePlan) -> Vec<&'static str>` | 未 provided 的方法名列表 |

### 2.3 计算规则

- `slot_index` 来自 `stdlib_trait_method_index()`（Stage 5.37）
- `method_name` 来自 `stdlib_trait_methods()` slice
- `provided` = `provided_method_names.contains(&method.name)`
- marker traits → `Some(StdlibVtablePlan { entries: vec![] })`（空 plan）
- 未注册 trait → `None`

### 2.4 命名标准化（§23）

| API/类型 | 命名规则 | 合规 |
|----------|---------|------|
| `StdlibVtablePlan` | `<Noun><Noun><Noun>` | ✅ |
| `StdlibVtablePlanEntry` | `<Noun><Noun><Noun><Noun>` | ✅ |
| `stdlib_vtable_plan` | `<noun>_<noun>_<noun>` | ✅ |
| `stdlib_vtable_plan_entry_count` | `<noun>_<noun>_<noun>_<noun>_<noun>` | ✅ |
| `stdlib_vtable_plan_is_complete` | `<noun>_<noun>_<noun>_<adj>` | ✅ |
| `stdlib_vtable_plan_missing_methods` | `<noun>_<noun>_<noun>_<adj>_<noun>` | ✅ |
| `slot_index` (field) | `<noun>_<noun>` | ✅ |
| `method_name` (field) | `<noun>_<noun>` | ✅ |
| `provided` (field) | `<adj>` (bool field) | ✅ |
| `trait_name` (field) | `<noun>_<noun>` | ✅ |
| `entries` (field) | `<noun>` | ✅ |

### 2.5 §16 接口隔离

`StdlibVtablePlan` / `StdlibVtablePlanEntry` 仅依赖 `&'static str` +
`Vec<>` + 标量字段，不引用 `mir::ty` / `codegen::EmitType` /
`traits::TraitResolver`，无循环依赖。所有查询函数是纯函数。

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过（基线 1172 + 新增 ~14 = ~1186）
4. §1.2 交付前验收：全绿

## 4. 测试矩阵

| 测试 | 描述 |
|------|------|
| `test_stdlib_vtable_plan_clone_complete` | Clone + [clone, clone_from] → 全 provided |
| `test_stdlib_vtable_plan_clone_partial` | Clone + [clone] → clone_from not provided |
| `test_stdlib_vtable_plan_drop` | Drop + [drop] → 1 entry provided |
| `test_stdlib_vtable_plan_partial_eq` | PartialEq + [eq] → ne not provided |
| `test_stdlib_vtable_plan_add` | Add + [add] → complete |
| `test_stdlib_vtable_plan_marker` | Copy + [] → 空 plan, is_complete=true |
| `test_stdlib_vtable_plan_unknown_trait` | BogusTrait → None |
| `test_stdlib_vtable_plan_extra_provided_ignored` | Clone + [clone, bogus] → bogus 不影响 plan |
| `test_stdlib_vtable_plan_entry_count` | Clone=2, Drop=1, Copy=0 |
| `test_stdlib_vtable_plan_is_complete_true` | Clone + 全方法 → true |
| `test_stdlib_vtable_plan_is_complete_false` | Clone + 部分 → false |
| `test_stdlib_vtable_plan_missing_methods_empty` | 完整 plan → 空 Vec |
| `test_stdlib_vtable_plan_missing_methods_partial` | Clone + [clone] → ["clone_from"] |
| `test_stdlib_vtable_plan_deterministic_order` | 重复调用顺序一致 |

## 5. 后续依赖

- **Stage 5.40+ (dyn Trait codegen)**: codegen 调用 `stdlib_vtable_plan()`
  一次，遍历 plan entries 直接生成 LLVM IR：
  - `provided=true` → 填入 `@landin_<Type>_<method>` symbol
  - `provided=false` → 填入 `null` 或 panic stub
- **Stage 5.41+ (typeck impl completeness)**: 调用
  `stdlib_vtable_plan_is_complete()` / `stdlib_vtable_plan_missing_methods()`
  报告"impl 未实现 trait 的 X / Y 方法"

---

**创建日期**: 2026-07-23
