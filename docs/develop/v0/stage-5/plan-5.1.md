# Stage 5 开发计划：TraitResolver 基础

> **阶段**: Stage 5.1
> **版本**: v0.10.2 → v0.11.0
> **状态**: 🔄 In progress
> **流程**: stage-committee-process.md v3.18 §17.3 时期 1

## 1. 目标

实现 TraitResolver 基础：收集 trait 定义 + impl 块 + 方法分派表。
这是 Stage 5 的核心功能，解锁 L5 trait dispatch。

## 2. 背景

当前编译器没有 TraitResolver：
- `ty_is_copy` 将所有 Adt 视为 Copy（pragmatic workaround）
- 方法调用（`x.method()`）无法解析到 trait impl 方法
- 没有 vtable 生成

Stage 5.1 实现基础设施：
- `TraitResolver` 结构 — 收集 trait 定义 + impl 块
- `ImplMap` — 从 (trait_def_id, self_ty_def_id) → impl_def_id
- `MethodMap` — 从 (trait_def_id, method_name) → fn_def_id
- `ty_is_copy` 使用 TraitResolver 判断 Copy（而非全部视为 Copy）

## 3. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 5.1-a | 创建 `src/traits/mod.rs` — TraitResolver 结构 | L2 |
| 5.1-b | 遍历 HIR owners 收集 trait 定义 + impl 块 | L2 |
| 5.1-c | 构建 ImplMap + MethodMap | L2 |
| 5.1-d | 在 driver 中集成 TraitResolver | L1 |
| 5.1-e | 添加测试 | L1 |

## 4. 验收标准

1. `cargo build` 0 warnings
2. `cargo clippy --all-targets` 0 warnings
3. `cargo fmt --check` 通过
4. 至少 3 个新测试
5. §17.3 三阶段文档协议执行（含 v3.18 docs/worklog.md 同步）

---

**创建日期**: 2026-07-22
