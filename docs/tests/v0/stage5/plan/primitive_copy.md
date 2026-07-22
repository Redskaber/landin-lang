# Stage 5.11 测试计划：primitive Copy 自动检测

> **阶段**: Stage 5.11
> **对应代码**: tests/v0/stage5/plan/primitive_copy_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `is_primitive_copy_kind()` 正确识别 always-Copy 的 MIR `TyKind` 变体，
并拒绝非 Copy 类型。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| 全部 primitive Copy kinds | test_all_primitive_copy_kinds_are_copy | ✅ | 10 个 kind 都返回 true |
| 带字段的 Int 变体 | test_int_variants_are_copy | ✅ | "Int(I32)" → 剥离 → "Int" → true |
| 非 Copy kinds 拒绝 | test_non_copy_kinds_rejected | ✅ | Str/Slice/Closure/Param/Adt 等 → false |
| Adt 带字段拒绝 | test_adt_with_fields_rejected | ✅ | "Adt(DefId(0))" → false |
| 未知 kind 拒绝 | test_unknown_kinds_rejected | ✅ | ""/Unknown/Vector → false |
| 常量数量校验 | test_primitive_copy_kinds_count | ✅ | BUILTIN_PRIMITIVE_COPY_KINDS.len() == 10 |

## 3. 测试统计

- 预期: 6, 实际: 6 (943 → 949, +6 ✅)

---

**创建日期**: 2026-07-22
