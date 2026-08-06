# Stage 16.84 测试计划 — Migrate checker.rs Type Errors to Use Resolver

> **阶段**: Stage 16.84
> **对应代码**: src/typeck/checker.rs + src/typeck/unify.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 checker.rs 类型错误使用 resolver-backed 类型名。

## 2. 覆盖场景

| 场景 | 测试函数名 | 极性 | 状态 | 说明 |
|------|-----------|------|------|------|
| format_ty 有 resolver | stage16_84_format_ty_with_resolver_shows_name | positive | ✅ PASS | 显示 "MyStruct" |
| format_ty 无 resolver | stage16_84_format_ty_without_resolver_falls_back | positive | ✅ PASS | fallback "i32" |
| expected function | stage16_84_compile_expected_function_found_struct_shows_name | negative | ✅ PASS | 含 "MyStruct" |
| if condition | stage16_84_compile_if_condition_must_be_bool_shows_name | negative | ✅ PASS | 含 "MyStruct" |
| switch discriminant | stage16_84_compile_switch_discriminant_shows_name | negative | ✅ PASS | 含 "MyStruct" |
| match arm | stage16_84_compile_match_arm_mismatch_shows_name | negative | ✅ PASS | 含类型名 |
| call non-function | stage16_84_compile_call_non_function_shows_name | negative | ✅ PASS | 含 "MyStruct" |
| method call | stage16_84_compile_method_call_non_function_shows_name | negative | ✅ PASS | method error |
| 全量回归 | cargo test | both | ✅ PASS | 2899 tests, 0 failures |

## 3. 测试统计

- 预期测试数: 8 (new) + 全量回归
- 实际测试数: 8 + 2891 existing = 2899
- 新增正向: 2
- 新增负向: 6
- 新增比例: 2:6 = 1:3 ✓
- 覆盖率: 100%

## 4. 依赖

- Stage 16.80 (type_kind_to_string_with_resolver)
- Stage 16.81 (UnificationTable::set_resolver)

## 5. 结论

全部 8 个新测试通过，2899 全量测试 0 failures，0 warnings。
