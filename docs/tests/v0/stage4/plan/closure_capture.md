# Stage 4.7 测试计划：闭包捕获分析

> **阶段**: Stage 4.7
> **对应代码**: tests/v0/stage4/plan/closure_capture_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证闭包捕获分析正确识别闭包体引用的外部变量，并将它们作为字段填充到
闭包的捕获环境结构体中。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 无捕获的闭包 | test_closure_no_captures | ✅ PASS | `\|x: i32\| x + 1` — 空环境 |
| 捕获单个变量 | test_closure_captures_one_var | ✅ PASS | `let y = 10; \|x: i32\| x + y` |
| 捕获多个变量 | test_closure_captures_multiple_vars | ✅ PASS | `let a = 1; let b = 2; \|x\| x + a + b` |
| 闭包不捕获参数 | test_closure_params_not_captured | ✅ PASS | 参数不算捕获 |

## 3. 测试统计

- 预期测试数: 4
- 实际测试数: 4
- 覆盖率: 100%

## 4. 依赖

- Stage 4.4: 闭包 lowering 基础（`AggregateKind::Closure`）
- `collect_captured_locals` 函数（Stage 4.7 新增）

---

**最后更新**: 2026-07-22 (Stage 4.7 完成)
