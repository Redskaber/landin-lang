# Stage 4.9 开发计划：L3 闭包调用 lowering

> **阶段**: Stage 4.9
> **版本**: v0.9.5 → v0.9.6
> **状态**: 🔄 In progress
> **流程**: stage-committee-process.md v3.17 §17.3 时期 1

## 1. 目标

实现闭包调用 lowering：当 `Call` 的 `func` 类型为 `TyKind::Closure` 时，
正确处理闭包调用语义——提取捕获环境 + 传递参数 + 调用闭包体。

## 2. 背景

Stage 4.4-4.7 实现了闭包 lowering + 捕获分析：
- `HirExprKind::Closure` → `AggregateKind::Closure` + 捕获环境
- `TyKind::Closure(def_id, capture_tys)` 携带捕获类型

当前 `Call` lowering 检查 `TyKind::Adt`（struct/enum ctor）和 `TyKind::FnDef`（普通函数），
但**不识别** `TyKind::Closure`——闭包调用会落入"real function call"分支，
生成错误的 `Terminator::Call`。

## 3. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 4.9-a | 在 `Call` lowering 中检测 `TyKind::Closure` | L1 |
| 4.9-b | 闭包调用语义：inline 闭包体到当前 MIR（简化方案） | L2 |
| 4.9-c | 添加测试 — 验证闭包调用不崩溃 + 产生正确 MIR | L1 |

## 4. 简化方案

由于完整的闭包调用需要独立函数 + 闭包环境传递（复杂），Stage 4.9 采用简化方案：
- 检测 `TyKind::Closure` 的 Call
- 不生成 `Terminator::Call`（闭包不是 FnDef）
- 而是生成一个返回 unit 的 local（占位）
- 标记为 Stage 4.10+ 完整闭包调用的基础

## 5. 验收标准

1. `cargo build` 0 warnings
2. `cargo clippy --all-targets -- -D warnings` 通过
3. `cargo fmt --check` 通过
4. 至少 2 个新测试
5. §17.3 三阶段文档协议执行

---

**创建日期**: 2026-07-22
