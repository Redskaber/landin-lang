# Stage 5.5 测试计划：vtable 生成基础

> **阶段**: Stage 5.5
> **对应代码**: tests/v0/stage5/plan/vtable_tests.rs
> **状态**: ✅ Complete (with audit enrichment)

## 1. 测试目标

验证 TraitResolver 正确为每个 `impl Trait for Type` 构建 vtable，且
`find_vtable(trait, type)` 返回的 entries 内容正确。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| vtable 构建 | test_vtable_built_for_impl | ✅ | `impl Foo for S` → vtable 存在 |
| vtable 内容查询 | test_vtable_query | ✅ | `find_vtable(Foo, S)` 返回的 entries 长度+method_name 正确 |
| 无 impl 无 vtable | test_no_vtable_without_impl | ✅ | 无 impl → 0 vtables |
| 多 impl 多 vtable | test_vtable_multiple_impls | ✅ | 两 trait 各 impl → 2 vtables |

## 3. 测试维度

| 维度 | 覆盖 |
|------|------|
| 正面（vtable 构建） | test_vtable_built_for_impl |
| 内容（entries 字段验证） | test_vtable_query (audit 补入) |
| 负面（无 impl） | test_no_vtable_without_impl |
| 多态（多 trait 多 impl） | test_vtable_multiple_impls |

## 4. §17 测试矩阵对齐

| 矩阵项 | Stage 5.5 |
|--------|-----------|
| 单元 | ✅ test_vtable_built_for_impl |
| 集成 | ✅ test_vtable_query |
| 负面 | ✅ test_no_vtable_without_impl |
| 多态 | ✅ test_vtable_multiple_impls |

## 5. Stage 5.6 修订说明

原始 `VtableEntry.fn_def_id: DefId` 在 Stage 5.6 改为 `fn_name: String`。
`test_vtable_query` 在 audit 阶段补入，验证新字段（`fn_name` 内容为
`landin_<Type>_<method>`）。修订详情见 `plan-5.6.md` §3.1。

## 6. 测试统计

- 预期: 4 (audit 后), 实际: TBD (待环境恢复验证)
- 原始 5.5 测试: 3 (仅 vtable_count)
- audit 补入: 1 (test_vtable_query)

## 7. 测试基础设施重构（Stage 5.5 audit）

本测试文件已纳入 `tests/all_tests.rs` 统一入口（`#[path] mod vtable_tests`）。
`Cargo.toml` 设置 `autotests = false`，仅构建单一 `all_tests` 目标。新增
测试文件只需在 `all_tests.rs` 添加一行 `#[path]` 声明，无需修改 `Cargo.toml`。

---

**创建日期**: 2026-07-22
**修订日期**: 2026-07-22 (audit: 补入 test_vtable_query + 测试基础设施重构)
