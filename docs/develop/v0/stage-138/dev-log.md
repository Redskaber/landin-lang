# Stage 138 开发日志 — TD-PTR-INDEX-CONST (*const T 索引)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.666.0 → v0.667.0 |
| 测试数 | 5822 (898 lib + 4924 integration, +3 stage138) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 138 修复 TD-PTR-INDEX-CONST — `*const T` 不支持 `[index]` 索引操作。

### WHY (根因分析)
- **根因**: 两处代码捕获 `RawPtr` 类型并报 "cannot index" 错误:
  1. `src/typeck/infer.rs:350` — typeck 的 `infer_projection` 不处理 `RawPtr`
  2. `src/mir/lower/field_resolution.rs:407` — MIR lower 的 `resolve_index_element_type` 不处理 `RawPtr`
- **通解**: 在两处 match 中添加 `TyKind::RawPtr(_, inner) => Some((**inner).clone())` 分支

### HOW (实施策略)
- `src/typeck/infer.rs`: 在 `ProjectionElem::Index` 的 match 中添加 `RawPtr` 分支
- `src/mir/lower/field_resolution.rs`: 在 `resolve_index_element_type` 的 match 中添加 `RawPtr` 分支
- 两处都返回 inner type（指针指向的类型），与 `*mut T` 行为一致

### §3.2 验收
- fmt clean, 0 clippy warnings
- 898 lib + 4924 integration = 5822 tests, 0 failures, 12 ignored

## Stage Summary
- Stage 138 PASSED — *const T 索引支持
- 3 tests: 1 positive + 1 regression + 1 array regression
- 0 regression (5819 → 5822 tests, +3 new)
- TD-PTR-INDEX-CONST ✅ 已修复
- v0.667.0
