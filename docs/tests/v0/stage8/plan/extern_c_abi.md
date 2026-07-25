# Stage 8.3 测试计划: extern "C" ABI support (§13.2)

> **阶段**: Stage 8.3
> **对应代码**: tests/v0/stage8/plan/extern_c_abi_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 §13.2 extern "C" ABI 支持 — `BodyMeta.abi` 字段贯穿 HIR → MIR → codegen
全管线。MVP 行为: Landin ABI 与 C ABI 使用相同 LLVM calling convention
(C 是 LLVM 默认 CC)，ABI 信息已 tracked 但尚未在 IR 中区分 (future: custom CC)。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| extern C fn 声明 | test_extern_c_fn_declaration | ✅ | `extern "C" fn f() {}` 解析成功 |
| extern C fn 调用 | test_extern_c_fn_call | ✅ | 调用 extern "C" fn 不报错 |
| 回归 (Landin ABI) | test_regression_landin_abi_still_works | ✅ | 默认 ABI 仍工作 |
| void fn | test_extern_c_void_fn | ✅ | `extern "C" fn f() -> ()` 处理 |
| 无参 fn | test_extern_c_no_param_fn | ✅ | `extern "C" fn f()` 处理 |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 8.3 测试 |
|--------|----------------|
| 正面 (extern C) | ✅ test_extern_c_fn_declaration + test_extern_c_fn_call |
| 边界 (void/无参) | ✅ test_extern_c_void_fn + test_extern_c_no_param_fn |
| 回归 | ✅ test_regression_landin_abi_still_works |

## 4. 测试统计

- 预期: 5, 实际: 5 (2062 → 2067, +5 ✅)
- v0.2 P2 extern "C" ABI tracking complete

---

**创建日期**: 2026-07-25
