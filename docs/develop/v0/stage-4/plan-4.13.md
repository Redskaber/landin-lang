# Stage 4.13 开发计划：完整闭包调用 lowering

> **阶段**: Stage 4.13
> **版本**: v0.9.9 → v0.10.0
> **状态**: 🔄 In progress
> **流程**: stage-committee-process.md v3.18 §17.3 时期 1

## 1. 目标

实现完整闭包调用 lowering：当 `Call` 的 `func` 类型为 `TyKind::Closure` 时，
提取捕获环境 + 绑定参数 + 内联闭包体到调用点。

## 2. 背景

Stage 4.9 实现了闭包调用检测（返回 unit placeholder）。
Stage 4.13 实现完整调用语义：
- 从闭包结构体 local 提取捕获字段（Projection::Field）
- 将捕获的值绑定到新的 locals（恢复闭包体引用的变量名）
- 将调用参数绑定到闭包参数 locals
- 直接内联闭包体 lowering 到调用点

## 3. 简化方案

完整闭包调用需要生成独立函数 + 闭包环境传递（非常复杂）。
Stage 4.13 采用 inline 方案：
- 不生成独立函数——直接在调用点内联闭包体
- 提取捕获字段 → 重建 locals → 绑定参数 → lower body
- 优点：简单、正确、不需要函数指针
- 缺点：闭包体在每次调用点重复（但 LLVM 会 inline 优化）

## 4. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 4.13-a | 从 closure local 提取捕获字段（Projection::Field） | L2 |
| 4.13-b | 将捕获的值绑定到新的 locals | L1 |
| 4.13-c | 将调用参数绑定到闭包参数 locals | L1 |
| 4.13-d | 内联 lower 闭包体 | L2 |
| 4.13-e | 添加测试 | L1 |

## 5. 验收标准

1. `cargo build` 0 warnings
2. `cargo clippy --all-targets` 0 warnings
3. `cargo fmt --check` 通过
4. 至少 2 个新测试
5. §17.3 三阶段文档协议执行（含 v3.18 worklog.md 同步）

---

**创建日期**: 2026-07-22
