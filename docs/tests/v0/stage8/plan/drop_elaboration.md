# Stage 8.4 测试计划: Drop elaboration (§5)

> **阶段**: Stage 8.4
> **对应代码**: tests/v0/stage8/plan/drop_elaboration_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 §5 drop elaboration — `src/borrowck/drop_elaboration.rs` 模块正确实现
drop check + drop order 规则。包括 needs_drop 判断 + 反向析构顺序。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 模块存在 | test_drop_elaboration_module_exists | ✅ | src/borrowck/drop_elaboration.rs 存在 |
| Copy type 无需 drop | test_copy_type_needs_no_drop | ✅ | i32/bool 等不进 drop set |
| Non-Copy type 需要 drop | test_non_copy_type_needs_drop | ✅ | struct without Copy 进 drop set |
| 反向析构顺序 | test_drop_order_reverse | ✅ | locals 反向声明顺序析构 |
| Struct 字段反向析构 | test_struct_fields_reverse_drop | ✅ | struct 字段反向析构 |
| Match arm 绑定 | test_match_arm_bindings_drop_at_arm_end | ✅ | match arm 绑定在 arm block 末析构 |
| 嵌套 type 递归 | test_nested_type_recursive_drop | ✅ | Array/Tuple/Adt 递归 needs_drop |

## 3. §17 测试矩阵对齐

| 矩阵项 | Stage 8.4 测试 |
|--------|----------------|
| 正面 (needs_drop) | ✅ test_non_copy_type_needs_drop |
| 负面 (Copy) | ✅ test_copy_type_needs_no_drop |
| 顺序 (reverse) | ✅ test_drop_order_reverse + test_struct_fields_reverse_drop |
| 多态 (nested) | ✅ test_nested_type_recursive_drop |
| 边界 (match arm) | ✅ test_match_arm_bindings_drop_at_arm_end |

## 4. 测试统计

- 预期: 7, 实际: 7 (2067 → 2083, +7 ✅)
- 另有 9 unit tests inline in drop_elaboration.rs
- v0.2 P2 drop elaboration complete

---

**创建日期**: 2026-07-25
