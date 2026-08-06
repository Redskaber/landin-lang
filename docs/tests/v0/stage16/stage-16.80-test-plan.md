# Stage 16.80 测试计划 — Improved Error Messages: Adt Type Names

> **阶段**: Stage 16.80
> **对应代码**: src/mir/ty.rs + src/typeck/error.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 resolver-backed 类型名解析正确工作，旧 API 不破坏。

## 2. 覆盖场景

| 场景 | 测试函数名 | 极性 | 状态 | 说明 |
|------|-----------|------|------|------|
| Adt 解析为 struct 名 | stage16_80_adt_resolves_name | positive | ✅ PASS | MyStruct → "MyStruct" |
| 原始类型不受影响 | stage16_80_primitive_unchanged | positive | ✅ PASS | i32 → "i32" |
| 未知 Adt 显示 ID | stage16_80_unknown_adt_shows_id | negative | ✅ PASS | DefId(9999) → "<adt#9999>" |
| mismatch 显示 struct 名 | stage16_80_mismatch_shows_struct_name | negative | ✅ PASS | 消息含 "MyStruct" + "i32" |
| mismatch 显示 enum 名 | stage16_80_mismatch_shows_enum_name | negative | ✅ PASS | 消息含 "MyEnum" |
| mismatch 完整消息 | stage16_80_mismatch_struct_vs_int_full_message | negative | ✅ PASS | "expected Foo, found i32" |
| mismatch 两个 struct | stage16_80_mismatch_two_structs | negative | ✅ PASS | "expected Foo, found Bar" |
| Param 显示参数名 | stage16_80_param_shows_name | negative | ✅ PASS | T → "T" |
| 旧 API 回归 | 全量测试 | both | ✅ PASS | 2867 tests, 0 failures |

## 3. 测试统计

- 预期测试数: 8 (new) + 全量回归
- 实际测试数: 8 + 2859 existing = 2867
- 新增正向: 2
- 新增负向: 6
- 新增比例: 2:6 = 1:3 ✓
- 覆盖率: 100%

## 4. 依赖

- Stage 15.80 (type_kind_to_string 基础)
- Stage 16.07-16.11 (Task 3: TraitResolver DefId-keyed lookup)

## 5. 结论

全部 8 个新测试通过，2867 全量测试 0 failures，0 warnings。
