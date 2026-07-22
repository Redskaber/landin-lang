# Stage 5.3 开发计划：改进 ty_is_copy — 使用 TraitResolver 检测 Copy trait

> **阶段**: Stage 5.3
> **版本**: v0.11.1 → v0.11.2
> **状态**: 🔄 In progress
> **流程**: stage-committee-process.md v3.18 §17.3 时期 1

## 1. 目标

改进 `ty_is_copy` 函数：使用 TraitResolver 检查类型是否实现了 Copy trait，
替代当前"所有 Adt 视为 Copy"的 pragmatic workaround（TD-016）。

## 2. 背景

当前 `borrowck::ty_is_copy` 将所有 `TyKind::Adt` 视为 Copy（Stage 3.40 workaround）。
这意味着非 Copy 类型（如 `String`、`Vec`）也被错误地视为 Copy，可能导致
use-after-move 检查不生效。

Stage 5.3 改进：
- 新增 `ty_is_copy_with_resolver(ty, resolver)` 函数 — 接受 TraitResolver
- 对于 `TyKind::Adt(def_id, _)`，检查 resolver 是否有 `impl Copy for <type>`
- 如果有 Copy impl → Copy ✅
- 如果没有 Copy impl → 非 Copy ❌
- 保留原始 `ty_is_copy` 作为回退（无 resolver 时使用）

## 3. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 5.3-a | 新增 `ty_is_copy_with_resolver(ty, &TraitResolver)` 函数 | L2 |
| 5.3-b | 在 borrowck 中使用新函数（如果 resolver 可用） | L1 |
| 5.3-c | 添加测试 | L1 |

## 4. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过
4. §17.3 三阶段文档协议执行

---

**创建日期**: 2026-07-22
