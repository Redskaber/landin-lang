# Stage 16.83 测试计划 — Diagnostic Type Name Resolution via Resolver

> **阶段**: Stage 16.83
> **对应代码**: src/driver.rs (to_diagnostics_with_resolver + format_via_diagnostics_with_resolver)
> **状态**: ✅ Complete

## 1. 测试目标

验证 diagnostic notes 使用 resolver-backed 类型名。

## 2. 覆盖场景

| 场景 | 测试函数名 | 极性 | 状态 | 说明 |
|------|-----------|------|------|------|
| 有 resolver | stage16_83_diagnostic_with_resolver_shows_struct_name | positive | ✅ PASS | notes 含 "MyStruct" |
| 无 resolver fallback | stage16_83_diagnostic_without_resolver_falls_back | positive | ✅ PASS | fallback 正常 |
| mismatch note | stage16_83_compile_mismatch_diagnostic_note_shows_name | negative | ✅ PASS | notes 含类型名 |
| struct full | stage16_83_compile_struct_mismatch_diagnostic_full | negative | ✅ PASS | "Foo" 显示 |
| enum | stage16_83_compile_enum_mismatch_diagnostic_shows_name | negative | ✅ PASS | "MyEnum" 显示 |
| two structs | stage16_83_compile_two_struct_diagnostic_shows_both | negative | ✅ PASS | "Foo"+"Bar" |
| fn arg | stage16_83_compile_fn_arg_diagnostic_shows_name | negative | ✅ PASS | 消息含 "MyStruct" |
| format output | stage16_83_format_for_user_with_resolver_shows_name | negative | ✅ PASS | 输出含名 |
| 全量回归 | cargo test | both | ✅ PASS | 2891 tests, 0 failures |

## 3. 测试统计

- 预期测试数: 8 (new) + 全量回归
- 实际测试数: 8 + 2883 existing = 2891
- 新增正向: 2
- 新增负向: 6
- 新增比例: 2:6 = 1:3 ✓
- 覆盖率: 100%

## 4. 依赖

- Stage 16.80 (type_kind_to_string_with_resolver)
- Stage 16.07-16.11 (Task 3: TraitResolver DefId-keyed lookup)

## 5. 结论

全部 8 个新测试通过，2891 全量测试 0 failures，0 warnings。
