# Stage 8.5 测试计划: async/await foundation (§10 MVP synchronous)

> **阶段**: Stage 8.5
> **对应代码**: tests/v0/stage8/plan/async_await_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 §10 async/await MVP — AST/HIR/Parser/MIR/Resolve 全管线支持 `async { block }`
和 `await expr` 语法。MVP 行为: 同步求值 (async block 同步执行, await expr 同步求值)。
Future: 状态机变换实现真正异步执行。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| AST variant 存在 | test_ast_async_await_variants_exist | ✅ | Expr::Await + Expr::Async 存在 |
| HIR variant 存在 | test_hir_async_await_variants_exist | ✅ | HirExprKind::Await + ::Async 存在 |
| Parser: async block | test_parser_async_block | ✅ | `async { block }` 解析为 Expr::Async |
| Parser: await expr | test_parser_await_expr | ✅ | `await expr` 解析为 Expr::Await |
| MVP 同步求值 | test_mvp_synchronous_evaluation | ✅ | async/await 在 pipeline 中无 panic |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 8.5 测试 |
|--------|----------------|
| AST 完整性 | ✅ test_ast_async_await_variants_exist |
| HIR 完整性 | ✅ test_hir_async_await_variants_exist |
| Parser 正面 | ✅ test_parser_async_block + test_parser_await_expr |
| Pipeline 集成 | ✅ test_mvp_synchronous_evaluation |

## 4. 测试统计

- 预期: 5, 实际: 5 (2083 → 2091, +5 ✅)
- 另有 3 unit tests inline in async_marker.rs
- v0.2 P3 async/await MVP complete (synchronous semantics)

---

**创建日期**: 2026-07-25
