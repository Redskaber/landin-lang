# Stage 4.9 测试计划：闭包调用 lowering

> **阶段**: Stage 4.9
> **对应代码**: tests/v0/stage4/plan/closure_call_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证闭包调用 lowering 正确识别 `TyKind::Closure` 的 Call，不生成错误的
`Terminator::Call`，且不崩溃。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 闭包定义后调用 | test_closure_call_no_crash | ✅ PASS | `let f = \|x\| x; f(42);` 不崩溃 |
| 闭包带捕获调用 | test_closure_call_with_capture | ✅ PASS | `let y = 10; let f = \|x\| x + y; f(1);` 不崩溃 |

## 3. 测试统计

- 预期测试数: 2
- 实际测试数: 2
- 覆盖率: 100%

---

**最后更新**: 2026-07-22 (Stage 4.9 完成)
