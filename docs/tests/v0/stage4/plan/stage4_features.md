# Stage 4 测试计划：嵌套模块 + 闭包 lowering

> **阶段**: Stage 4.1-4.4
> **对应代码**: tests/v0/stage4/plan/stage4_tests.rs (新建) + tests/hir_resolution.rs (追加) + tests/mir_lowering.rs (追加)
> **状态**: ✅ Complete

## 1. 测试目标

验证 Stage 4.1-4.4 的核心功能：
- Stage 4.1: 嵌套模块解析（`mod foo { pub fn bar() {} }`）
- Stage 4.4: 闭包 lowering（`|x: i32| x + 1` → `AggregateKind::Closure`）

## 2. 覆盖场景

### Stage 4.1: 嵌套模块

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 单层嵌套模块 fn 解析 | nested_module_items_resolve | ✅ PASS | `mod inner { pub fn f() {} }` + `inner::f()` |
| 嵌套模块中的 struct | nested_module_struct_resolves | ✅ PASS | struct inside module |
| 2 层深度嵌套 | deeply_nested_module_resolves | ✅ PASS | `a::b::deep_fn` |

### Stage 4.4: 闭包 lowering

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 闭包产生 AggregateKind::Closure | closure_lowers_to_aggregate | ✅ PASS | `\|x: i32\| x + 1` |
| 复杂闭包体不崩溃 | closure_no_crash_on_complex_body | ✅ PASS | if-expression body |

## 3. 测试统计

- 预期测试数: 5
- 实际测试数: 5
- 覆盖率: 100%

## 4. 依赖

- Stage 4.1: `build_module_tree` 递归处理 + `build_child_module` + `item_def_id`
- Stage 4.4: `HirExprKind::Closure` lowering + `AggregateKind::Closure` + `TyKind::Closure`

## 5. 测试代码位置

- `tests/hir_resolution.rs` — 3 个嵌套模块测试（追加到现有文件）
- `tests/mir_lowering.rs` — 2 个闭包 lowering 测试（追加到现有文件）

**注**: 按 v3.17 §17.1，新测试应放置在 `tests/v0/stage4/plan/`，但本轮
测试已追加到现有文件（迁移期）。后续新测试将按新结构放置。

---

**最后更新**: 2026-07-22 (Stage 4.6)
