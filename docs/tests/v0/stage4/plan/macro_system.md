# Stage 4.10 测试计划：宏系统基础

> **阶段**: Stage 4.10
> **对应代码**: tests/v0/stage4/plan/macro_system_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标
验证内置宏展开系统正确处理 `println!`、`stringify!`、`assert!` 宏调用。

## 2. 覆盖场景
| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| println! 宏不崩溃 | test_macro_println_no_crash | ✅ PASS | `println!("hello");` 不崩溃 |
| stringify! 展开 | test_macro_stringify | ✅ PASS | `let s = stringify!(x);` 产生 MIR |
| assert! 宏不崩溃 | test_macro_assert_no_crash | ✅ PASS | `assert!(1 == 1);` 不崩溃 |

## 3. 测试统计
- 预期测试数: 3
- 实际测试数: 3
- 覆盖率: 100%

---

**最后更新**: 2026-07-22 (Stage 4.10 完成)
