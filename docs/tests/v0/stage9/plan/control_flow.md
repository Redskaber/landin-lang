# Stage 9.3 测试计划: Control flow conformance expansion

> **阶段**: Stage 9.3
> **对应代码**: tests/v0/stage9/plan/control_flow_tests.rs + tests/conformance/00-parse/02-control-flow/*.lin
> **状态**: ✅ Complete

## 1. 测试目标

1. 验证 conformance `02-control-flow/` category 扩展 (1 → 80 .lin files)
2. 验证 §3.4 (control flow expressions) + §3.6 (stmt + block) + §3.4 (match_arm) 实现正确性
3. 验证 if-let / while-let 是 Stage 1 features (在 Stage 0 报错 "not yet supported")

## 2. Rust 集成测试 (tests/v0/stage9/plan/control_flow_tests.rs)

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| Control-flow 目录有 80 .lin | test_stage9_3_control_flow_directory_populated | ✅ | 80 files |
| if/else (12 tests) | test_stage9_3_if_else_tests_present | ✅ | if_basic through if_expr_returns |
| if-let (6 tests, all FAIL) | test_stage9_3_if_let_tests_marked_fail | ✅ | if-let is Stage 1 feature, all 6 marked FAIL with "Stage 0" pattern |
| while (8 tests) | test_stage9_3_while_tests_present | ✅ | while_basic through while_in_fn |
| while-let (5 tests, all FAIL) | test_stage9_3_while_let_tests_marked_fail | ✅ | while-let is Stage 1 feature |
| for (8 tests) | test_stage9_3_for_tests_present | ✅ | for_basic through for_empty |
| loop (6 tests) | test_stage9_3_loop_tests_present | ✅ | loop_basic through loop_while_interplay |
| match (15 tests) | test_stage9_3_match_tests_present | ✅ | match_basic through match_empty |
| break/continue/return (10 tests) | test_stage9_3_break_continue_return_tests_present | ✅ | break_basic through return_in_match |
| block/stmt (5 tests) | test_stage9_3_block_stmt_tests_present | ✅ | block_basic through stmt_let_with_type |
| 错误恢复 (5 tests, 4 FAIL) | test_stage9_3_error_recovery_tests_present | ✅ | err_if/match/while/for (FAIL) + err_break_outside_loop (PASS, parser recovery) |
| Stage 9.3 docs 创建 | test_stage9_3_docs_created | ✅ | plan-9.3.md + gate-review-9.3.md + control_flow.md |
| Cargo.toml 版本 bump | test_stage9_3_cargo_toml_version_bumped | ✅ | 0.16.2 |
| Conformance 总数 ≥ 177 | test_stage9_3_conformance_total_reaches_177 | ✅ | 98 + 79 = 177 |

## 3. Conformance .lin 测试 (tests/conformance/00-parse/02-control-flow/)

### 3.1 新增 79 个测试 (Stage 9.3)

| 类别 | 测试数 | 备注 |
|------|-------|------|
| if / else | 12 | if/else/else-if/nested/cmp/logic/call/multi-stmt/empty |
| if-let (FAIL — Stage 1 feature) | 6 | all marked FAIL with "not yet supported in Stage 0" pattern |
| while | 8 | basic/cmp/logic/empty/break/continue/nested/in-fn |
| while-let (FAIL — Stage 1 feature) | 5 | all marked FAIL with "not yet supported in Stage 0" pattern |
| for | 8 | basic/range/inclusive-range/break/continue/nested/tuple-pat/empty |
| loop | 6 | basic/break/break-value/continue/nested/while-interplay |
| match | 15 | basic/multi-arm/wildcard/ident/tuple/struct/enum/guard/block-arm/range/or-pat/nested/in-let/expr-scrutinee/empty |
| break/continue/return | 10 | break basic/value/in-while/in-for; continue basic/in-for/in-loop; return basic/void/in-match |
| block + stmt | 5 | basic/expr/trailing-expr/let/let-with-type |
| Error recovery | 4 FAIL + 1 PASS | err_if/match/while/for (FAIL — parser errors) + err_break_outside_loop (PASS — parser recovery) |

### 3.2 累计 conformance: 98 → 177 (+79 ✅)

## 4. §17 测试矩阵对齐

| 矩阵项 | Stage 9.3 测试 |
|--------|----------------|
| 正面 (PASS) | ✅ 64 .lin + 14 rust tests |
| 负面 (FAIL — parser errors) | ✅ 4 .lin (err_if_without_cond, err_match_without_scrutinee, err_while_without_cond, err_for_without_in) |
| Stage 1 features (FAIL — not yet supported) | ✅ 11 .lin (6 if-let + 5 while-let, all marked FAIL with "Stage 0" pattern) |
| 错误恢复 (PASS via synthetic node) | ✅ 1 .lin (err_break_outside_loop — parser accepts, semantic check at later stage) |
| 边界 (empty/nested) | ✅ if_empty_block, match_empty, for_empty, while_empty, match_nested, if_nested, while_nested, for_nested, loop_nested, if_let_chain, while_let_nested |
| 多态 (conditions/patterns) | ✅ if_cond_cmp, if_cond_logic, if_cond_call, match_tuple, match_struct, match_enum, match_guard, match_range_pat, match_or_pat, for_pat_tuple, if_let_tuple, while_let_tuple |

## 5. 测试统计

- 预期: 79 .lin + 14 rust = 93 new tests
- 实际: 79 .lin (11 converted PASS→FAIL after parser "not yet supported in Stage 0" message) + 14 rust = 93 new tests
- Conformance: 98 → 177 (+79 ✅)
- Rust integration: 2122 → 2136 (+14 ✅)
- 0 regressions

## 6. 关键发现

**Stage 1 features identified**: `if let` and `while let` are **not yet supported
in Stage 0** (per parser message: "will be added in Stage 1"). The parser
explicitly emits an error when encountering these constructs.

**Discovery outcome**:
- 6 `if-let` tests (if_let_basic, if_let_else, if_let_tuple, if_let_struct,
  if_let_wildcard, if_let_chain) — initially written as PASS, converted to FAIL
  with error_pattern "not yet supported in Stage 0"
- 5 `while-let` tests (while_let_basic, while_let_break, while_let_tuple,
  while_let_nested, while_let_continue) — same conversion

This is a positive outcome — the conformance suite clarified which control
flow features are Stage 0 vs Stage 1, providing clear scope for the v0.1
release gate.

**Parser recovery behavior**:
- `err_break_outside_loop` (`fn f() { break; }`) — PASS, parser accepts
  (semantic check at later stage); this differs from `err_if_without_cond`
  which produces "expected" error

---

**创建日期**: 2026-07-26
