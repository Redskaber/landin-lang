# Stage 18.87 — GATs Phase 3: Projection Resolver Bug Fixes + Complete Compound Type Coverage

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.354.0 → v0.355.0
> **Process**: stage-committee-process.md v5.0 §13.1 + §13.5 + §14
> **Status**: ✅ Complete

## 1. 背景

v0.7 路线图 P1 GATs 实现:
- Phase 1 (Stage 18.52): ✅ AST/Parser/HIR 基础设施
- Phase 2 (Stage 18.53): ✅ Qualified path 解析 + Projection lowering
- **Phase 3 (本 Stage)**: 投影解析器 bug 修复 + 完整覆盖

`projection_resolver.rs` 有 5 个已知 bug (B5-B9) 在 Stage 16.71 审计中识别:

| Bug | 描述 | 修复 |
|-----|------|------|
| B5 | `find_trait_for_assoc_type` DefId/HirId 不匹配 | 使用 hir_id.owner 正确匹配 |
| B6 | `resolve_projection_in_ty` 缺 FnDef/FnPtr/Closure | 添加这 3 个分支 |
| B7 | `types_match` 缺 14 TyKind variants | 补全所有 variants |
| B8 | 循环绑定无限递归风险 | 添加递归深度限制 |
| B9 | 应在 writeback_closures 之后运行 | 检查 driver 调用顺序 |

## 2. 修复方案

### 2.1 B6: 完整 compound type 覆盖

在 `resolve_projection_in_ty` 添加:
- `TyKind::FnDef(def_id, substs)` — 递归解析 substs
- `TyKind::FnPtr(sig)` — 递归解析 inputs + output
- `TyKind::Closure(def_id, substs)` — 递归解析 substs
- `TyKind::Projection(_, substs)` — 递归解析 substs (嵌套投影)

### 2.2 B7: 完整 `types_match` 覆盖

补全缺失的 variants:
- Float, Never, Tuple, Array, Slice, Ref, RawPtr, FnDef, FnPtr, Closure, Projection, Error, Infer, Foreign

### 2.3 B8: 递归深度限制

添加 `depth: u32` 参数，限制为 10 层。超过返回原始类型 (graceful degradation)。

## 3. §6.3 委员会投票

**5/5 GO** ✅
