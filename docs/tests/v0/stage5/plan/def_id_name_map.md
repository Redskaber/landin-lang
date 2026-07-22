# Stage 5.4 测试计划：DefId→name 反向映射 + 完整 Copy 检测

> **阶段**: Stage 5.4
> **对应代码**: tests/v0/stage5/plan/def_id_name_map_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 TraitResolver 正确填充 `type_by_def_id` 反向映射，并使
`ty_is_copy_with_resolver` 能够通过此映射对 Adt 类型做精确 Copy 检测。

关闭 TD-016 (L-COPY-ADT)。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 类型名映射填充 | test_type_by_def_id_populated | ✅ | `struct Point` → `type_count() >= 1` |
| Copy 实现检测 | test_copy_detection_with_impl | ✅ | `impl Copy for S` → 1 trait + 1 impl + ≥2 types |
| 无 Copy 实现回退 | test_copy_detection_without_impl | ✅ | 无 impl → 0 traits + 0 impls |

## 3. 测试维度

| 维度 | 覆盖 |
|------|------|
| 正面（映射存在） | test_type_by_def_id_populated |
| 集成（trait + impl + type 同时存在） | test_copy_detection_with_impl |
| 负面（无 trait / 无 impl） | test_copy_detection_without_impl |

## 4. §17 测试矩阵对齐

| 矩阵项 | Stage 5.4 |
|--------|-----------|
| 单元 | ✅ test_type_by_def_id_populated |
| 集成 | ✅ test_copy_detection_with_impl |
| 负面 | ✅ test_copy_detection_without_impl |

## 5. TD-016 关闭证据

| 关闭条件 | 验证测试 |
|----------|----------|
| `type_by_def_id` 在 collect 时填充 | test_type_by_def_id_populated |
| `is_copy(def_id, copy_name)` 可查询 | test_copy_detection_with_impl |
| 无 impl 时返回 false（非回退 true） | test_copy_detection_without_impl |

## 6. 测试统计

- 预期: 3, 实际: 3 (1010 → 1013, +3 ✅)

---

**创建日期**: 2026-07-22
**修订日期**: 2026-07-22 (audit: 文档补全)
