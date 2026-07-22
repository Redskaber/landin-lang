# Stage 4.12 开发计划：完整可见性强制 + Process v3.18

> **阶段**: Stage 4.12
> **版本**: v0.9.8 → v0.9.9
> **状态**: 🔄 In progress
> **流程**: stage-committee-process.md v3.18 §17.3 时期 1

## 1. 目标

1. 实现 `current_module` 跟踪——激活完整可见性强制（`pub`/`pub(crate)`/`pub(super)`/private）
2. 更新流程文档至 v3.18（worklog 快照同步）
3. 创建 `docs/worklog/` 目录结构 + 历史快照

## 2. 背景

Stage 4.3 实现了 `check_visibility` 但所有 same-crate 访问都允许（ADR-004）。
Stage 4.1 实现了嵌套模块支持（`ModuleNode.children` 填充）。
现在可以添加 `current_module` 跟踪来激活完整的跨模块可见性强制。

## 3. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 4.12-a | 在 resolver 中添加 `current_module: Option<Spur>` 跟踪 | L1 |
| 4.12-b | 在 `resolve_body` 时设置 `current_module` | L1 |
| 4.12-c | 更新 `check_visibility` 使用 `current_module` 做跨模块检查 | L2 |
| 4.12-d | 添加测试 | L1 |

## 4. 验收标准

1. `cargo build` 0 warnings
2. `cargo clippy --all-targets` 0 warnings
3. `cargo fmt --check` 通过
4. 至少 2 个新测试
5. §17.3 三阶段文档协议执行（含 v3.18 worklog 快照同步）

---

**创建日期**: 2026-07-22
