# Stage 16.82 测试计划 — BorrowError Message Improvements

> **阶段**: Stage 16.82
> **对应代码**: src/borrowck/mod.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 BorrowError 消息改进：显示实际类型名 + place 信息。

## 2. 覆盖场景

| 场景 | 测试函数名 | 极性 | 状态 | 说明 |
|------|-----------|------|------|------|
| format_ty 有 resolver | stage16_82_format_ty_with_resolver_shows_name | positive | ✅ PASS | 显示 "MyStruct" |
| format_ty 无 resolver | stage16_82_format_ty_without_resolver_falls_back | positive | ✅ PASS | 显示 "i32" |
| move after borrow | stage16_82_compile_move_after_borrow_shows_place | negative | ✅ PASS | 含 "local#" |
| immutable reassign | stage16_82_compile_assign_immutable_shows_local | negative | ✅ PASS | 含 "local#" |
| double mut borrow | stage16_82_compile_double_mut_borrow_shows_place | negative | ✅ PASS | 含错误 |
| use after move | stage16_82_compile_use_after_move_shows_place | negative | ✅ PASS | 含 "local#" |
| format_place | stage16_82_format_place_local | negative | ✅ PASS | 输出 "local#5" |
| format_place_path | stage16_82_format_place_path_local | negative | ✅ PASS | 输出 "local#3" |
| 全量回归 | cargo test | both | ✅ PASS | 2883 tests, 0 failures |

## 3. 测试统计

- 预期测试数: 8 (new) + 全量回归
- 实际测试数: 8 + 2875 existing = 2883
- 新增正向: 2
- 新增负向: 6
- 新增比例: 2:6 = 1:3 ✓
- 覆盖率: 100%

## 4. 依赖

- Stage 16.80 (type_kind_to_string_with_resolver)
- Stage 16.07-16.11 (Task 3: TraitResolver DefId-keyed lookup)
- Stage 14.106 (BorrowChecker with_resolver)

## 5. 结论

全部 8 个新测试通过，2883 全量测试 0 failures，0 warnings。
