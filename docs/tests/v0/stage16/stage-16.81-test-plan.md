# Stage 16.81 测试计划 — Migrate unify.rs to mismatch_with_resolver

> **阶段**: Stage 16.81
> **对应代码**: src/typeck/unify.rs + src/driver.rs + src/mir/ty.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 unify.rs 使用 resolver-backed 类型名，实际编译错误显示类型名而非 <adt>。

## 2. 覆盖场景

| 场景 | 测试函数名 | 极性 | 状态 | 说明 |
|------|-----------|------|------|------|
| unify 有 resolver | stage16_81_unify_with_resolver_shows_struct_name | positive | ✅ PASS | 错误含 "MyStruct" |
| unify 无 resolver fallback | stage16_81_unify_without_resolver_falls_back | positive | ✅ PASS | 错误含 "<adt>" |
| 编译 struct vs int | stage16_81_compile_mismatch_struct_int_shows_name | negative | ✅ PASS | 错误含 "MyStruct" |
| 编译两个 struct | stage16_81_compile_mismatch_two_structs_shows_names | negative | ✅ PASS | "Foo" + "Bar" |
| 编译 enum vs int | stage16_81_compile_mismatch_enum_int_shows_name | negative | ✅ PASS | 错误含 "MyEnum" |
| 编译 &struct | stage16_81_compile_mismatch_struct_ref_shows_name | negative | ✅ PASS | &MyStruct 名显示 |
| 编译 fn arg | stage16_81_compile_mismatch_fn_arg_shows_name | negative | ✅ PASS | 函数参数错误 |
| 编译 return type | stage16_81_compile_mismatch_return_type_shows_name | negative | ✅ PASS | 返回类型错误 |
| 全量回归 | cargo test | both | ✅ PASS | 2875 tests, 0 failures |

## 3. 测试统计

- 预期测试数: 8 (new) + 全量回归
- 实际测试数: 8 + 2867 existing = 2875
- 新增正向: 2
- 新增负向: 6
- 新增比例: 2:6 = 1:3 ✓
- 覆盖率: 100%

## 4. 依赖

- Stage 16.80 (type_kind_to_string_with_resolver + mismatch_with_resolver)
- Stage 16.07-16.11 (Task 3: TraitResolver DefId-keyed lookup)

## 5. 结论

全部 8 个新测试通过，2875 全量测试 0 failures，0 warnings。
