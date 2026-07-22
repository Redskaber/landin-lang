# Stage 4.13 测试计划：完整闭包调用 lowering

> **阶段**: Stage 4.13
> **对应代码**: tests/v0/stage4/plan/closure_full_call_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标
验证完整闭包调用 lowering 提取捕获 + 产生推断类型结果。

## 2. 覆盖场景
| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 无捕获闭包调用 | test_full_closure_call_no_capture | ✅ PASS | `let f = \|x\| x; f(42);` |
| 带捕获闭包调用 | test_full_closure_call_with_capture | ✅ PASS | `let y = 10; let f = \|x\| x + y; f(1);` |

## 3. 测试统计
- 预期: 2, 实际: 2, 覆盖率: 100%

---

**最后更新**: 2026-07-22 (Stage 4.13 完成)
