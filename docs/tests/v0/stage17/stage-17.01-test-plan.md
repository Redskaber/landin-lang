# Stage 17.01 测试计划 — CodegenError Error System Phase 1

> **阶段**: Stage 17.01
> **对应代码**: src/codegen/error.rs + src/codegen/llvm/helpers.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 CodegenError 类型和 cstr_result helper 正确工作。

## 2. 覆盖场景

| 场景 | 测试函数名 | 极性 | 状态 | 说明 |
|------|-----------|------|------|------|
| CodegenError 构造 | stage17_01_codegen_error_new_creates_error | positive | ✅ PASS | 正确构造 |
| cstr_result 有效字符串 | stage17_01_cstr_valid_string_returns_ok | positive | ✅ PASS | 返回 Ok |
| cstr_result NUL 字节 | stage17_01_cstr_nul_byte_returns_error | negative | ✅ PASS | 返回 Err |
| 错误消息 | stage17_01_codegen_error_message_correct | negative | ✅ PASS | 消息正确 |
| 错误 span | stage17_01_codegen_error_span_correct | negative | ✅ PASS | span 正确 |
| Result Ok 变体 | stage17_01_codegen_result_ok_variant | negative | ✅ PASS | Ok 正确 |
| Result Err 变体 | stage17_01_codegen_result_err_variant | negative | ✅ PASS | Err 正确 |
| cstr_result 空字符串 | stage17_01_cstr_empty_string_returns_ok | negative | ✅ PASS | 空字符串 Ok |
| 全量回归 | cargo test | both | ✅ PASS | 2952 tests, 0 failures |

## 3. 测试统计

- 新增正向: 2
- 新增负向: 6
- 新增比例: 2:6 = 1:3 ✓

## 4. 结论

全部 8 个新测试通过，2952 全量测试 0 failures，0 warnings。
