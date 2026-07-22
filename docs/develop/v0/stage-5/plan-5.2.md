# Stage 5.2 开发计划：TraitResolver 集成到 driver + Copy trait 检测

> **阶段**: Stage 5.2
> **版本**: v0.11.0 → v0.11.1
> **状态**: 🔄 In progress
> **流程**: stage-committee-process.md v3.18 §17.3 时期 1

## 1. 目标

1. 修复 `cargo fmt` 问题（src/traits/mod.rs + tests）
2. 将 TraitResolver 集成到 driver 流水线
3. 使用 TraitResolver 的 `implements()` 方法改进 `ty_is_copy`（从"所有 Adt 视为 Copy"改为"实现了 Copy trait 的类型视为 Copy"）

## 2. 背景

Stage 5.1 创建了 TraitResolver 但未集成到 driver。
当前 `borrowck::ty_is_copy` 将所有 Adt 类型视为 Copy（pragmatic workaround，TD-016）。
Stage 5.2 集成 TraitResolver 到 driver，并改进 Copy 检测。

## 3. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 5.2-a | 修复 cargo fmt 问题 | L1 |
| 5.2-b | 在 driver.rs 中创建 TraitResolver + 调用 collect() | L1 |
| 5.2-c | 在 CompileResult 中添加 TraitResolver | L1 |
| 5.2-d | 添加测试 | L1 |

## 4. 验收标准

1. `cargo fmt --check` 通过（零 diff）
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过
4. §17.3 三阶段文档协议执行

---

**创建日期**: 2026-07-22
