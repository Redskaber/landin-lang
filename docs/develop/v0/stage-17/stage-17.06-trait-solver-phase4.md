# Stage 17.06 — Trait Solver Phase 4 (Supertrait Expansion)

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.279.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

为 Trait Solver 添加 supertrait expansion — 评估 "Type: Trait" 时递归检查所有 supertraits。

## 2. 实现

### 2.1 重构 evaluate()

- `evaluate()` → `evaluate_implies()`: 先直接检查，Yes 后检查 supertraits
- `evaluate_direct()`: 原有逻辑（assumptions + resolver lookup）
- `evaluate_supertraits()`: 递归检查 supertraits，visited HashSet 防循环

### 2.2 Supertrait 查找

通过 `resolver.trait_supertraits(spur)` 获取 supertrait 名字 Spur 列表，
再用 `resolver.find_trait_def_id(spur)` 转为 DefId。

### 2.3 递归逻辑

- 任何 supertrait 为 No → 整体 No
- Ambiguous → 继续检查其他 supertrait（不立即返回）
- 全部 Yes → Yes

## 3. Trait Solver 完整架构 (Phase 1-4)

| Phase | Stage | 内容 |
|-------|-------|------|
| 1 | 17.03 | 数据结构 |
| 2 | 17.04 | where clause assumptions |
| 3 | 17.05 | driver integration |
| 4 | 17.06 | supertrait expansion |

## 4. 验收

| 命令 | 要求 | 实际 |
|------|------|------|
| cargo build --features llvm-backend | ✅ | ✅ |
| cargo fmt --check | ✅ | ✅ |
| cargo clippy --all-targets | 0 warnings | ✅ |
| cargo test | 0 failures | ✅ 431 lib + 2529 integration = 2960 |
