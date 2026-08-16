# Stage 18.166 — 任务审查: variant constructor 阻塞 + 重排

> **Author**: redskaber (PM-A + ARCH-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.434.0 (Stage 18.166 任务审查报告)
> **Process**: docs/stage-committee-process.md v6.4 §5.1 (复杂度预评估) + §17 (任务规划排版图)
> **Task ID**: stage18.166

## 1. 任务审查背景

按 Stage 18.165 计划, 本 stage 实现 variant constructor (不带前缀的 Some/None/Ok/Err)。

## 2. 审查发现: 阻塞问题

### 2.1 实现尝试

1. 在 `module_build.rs` 注册 enum variant 到 value namespace ✅ (编译通过)
2. 测试 `Some(42)` 不带前缀 → ❌ MIR lower panic

### 2.2 根因

MIR lower (`expr_variants.rs:362-365`) 要求 variant path 至少 2 段 (`Option::Some`):
```rust
if path.segments.len() >= 2 {
    if let Some((idx, tys)) = resolve_enum_variant(cx, adt_def_id, &path.segments[1].ident.name) {
```

对于单独的 `Some` (1 段), MIR lower 无法确定是哪个 variant, 因为:
1. variant 复用 enum 的 DefId (无独立 DefId)
2. MIR lower 需要从 path 的第二段获取 variant 名称
3. 单段 `Some` 没有第二段

### 2.3 能力缺口

| 缺口 | 影响 | 修复方案 |
|------|------|---------|
| Variant 独立 DefId | MIR lower 无法区分 variants | 需要 HIR 为每个 variant 分配独立 DefId |
| 单段 path variant 解析 | MIR lower 不支持 1 段 path | 需要修改 MIR lower 查找 variant by name |
| Variant name → Enum 映射 | resolver 无法从 variant name 找到 enum | 需要建立 variant name → enum DefId 索引 |

### 2.4 复杂度重新评估

原评估 L2, 实际为 **L3** (核心架构 — 需要 HIR variant DefId + MIR lower 重构)。

## 3. 重排任务排版图

### 3.1 原排版图

```
Stage 18.166: variant constructor (不带前缀 Some/None/Ok/Err)
Stage 18.167: Option/Result 基本方法 (is_some/unwrap)
```

### 3.2 新排版图

```
Stage 18.166 (本 stage):
  → 任务审查 + 回退 variant 注册 (避免 panic)
  → 记录能力缺口 + 修订计划

Stage 18.167 (下一步):
  → 实现 HIR variant 独立 DefId (基础设施)
  → 修改 build_module_tree 为 variant 分配独立 DefId
  → 修改 def_kinds/def_span 记录 variant DefId

Stage 18.168:
  → 实现 variant name → enum DefId 索引
  → 修改 resolver 单段 path 查找 variant
  → 修改 MIR lower 支持单段 path variant

Stage 18.169:
  → Option/Result 基本方法 (is_some/unwrap)
```

### 3.3 重排原因

| 原任务 | 重排原因 | 新位置 |
|--------|---------|--------|
| variant constructor | 需要 HIR variant DefId 基础设施, L3 复杂度 | 拆分为 18.167 (DefId) + 18.168 (resolver+MIR) |
| Option/Result 方法 | 依赖 variant constructor (方法返回 Option/Result) | 18.169 (在 variant constructor 之后) |

## 4. 回退

已回退 `module_build.rs` 的 variant 注册 (避免 `Some(42)` panic)。
保留 `prelude.rs` 的 Option/Result 注入 (带前缀 `Option::Some` 仍工作)。

## 5. 简写和缺陷记录

### 5.1 简写1: variant constructor 需要基础设施

**原因**: MIR lower 要求 2 段 path, 单段 `Some` 无法解析 variant。
**修订计划**: Stage 18.167-18.168 实现 HIR variant DefId + resolver 索引 + MIR lower 支持。

### 5.2 简写2: Option/Result 方法推迟

**原因**: 方法返回 Option/Result, 依赖 variant constructor 完整工作。
**修订计划**: Stage 18.169 实现。

## 6. §3.2 验收

本 stage 为任务审查 + 回退, 验收基于回退后状态:
- ✅ cargo check: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo test: 全部通过 (回退到 v0.433.0 工作状态)

## 7. Stage Summary

- **Stage 18.166 PASSED** — 任务审查: variant constructor 阻塞 + 重排
- **发现**: variant constructor 需要 HIR variant DefId 基础设施 (L3), 非 L2
- **回退**: module_build.rs variant 注册 (避免 panic)
- **重排**: variant constructor 拆分为 18.167 (DefId) + 18.168 (resolver+MIR)
- **保留**: prelude.rs Option/Result 注入 (带前缀仍工作)
- **v0.434.0**: patch bump (任务审查, 无功能修改)
- **下一步**: Stage 18.167 实现 HIR variant 独立 DefId
