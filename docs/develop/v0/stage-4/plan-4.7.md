# Stage 4.7 开发计划：L3 闭包捕获分析

> **阶段**: Stage 4.7
> **版本**: v0.9.3 → v0.9.4
> **状态**: 🔄 In progress
> **流程**: stage-committee-process.md v3.17 §17.3 时期 1

## 1. 目标

实现闭包捕获分析：检测闭包体引用的外部变量，将它们作为字段填充到闭包的
捕获环境结构体中。这是 L3 闭包 codegen 的核心功能。

## 2. 背景

Stage 4.4 实现了闭包 lowering 基础：
- `HirExprKind::Closure` → `AggregateKind::Closure(def_id, substs)`
- `TyKind::Closure` → `EmitType::Struct(vec![])` (空结构体)
- 捕获环境为空（no variables captured）

Stage 4.7 需要：
- 遍历闭包体，找出引用的外部变量（不在闭包参数中的变量）
- 将这些变量的类型作为闭包结构体的字段
- 将这些变量的值作为 `Aggregate` 的 operands

## 3. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 4.7-a | 实现 `collect_captured_locals` — 遍历闭包体找出外部变量引用 | L2 |
| 4.7-b | 修改 closure lowering — 将捕获变量类型填入 `TyKind::Closure` 的 substs | L2 |
| 4.7-c | 修改 closure lowering — 将捕获变量值填入 `Aggregate` 的 operands | L2 |
| 4.7-d | 修改 codegen — `TyKind::Closure` 根据捕获变量生成结构体类型 | L2 |
| 4.7-e | 添加测试 — 验证捕获变量出现在闭包结构体中 | L1 |

## 4. 验收标准

1. `cargo build` 0 warnings
2. `cargo clippy --all-targets -- -D warnings` 通过
3. `cargo fmt --check` 通过
4. 至少 2 个新测试验证捕获分析
5. §17.3 三阶段文档协议执行

## 5. 风险

- 捕获分析需要遍历 HirExpr，可能遗漏某些表达式类型
- 缓解：先支持最常见的变量引用（Path → Local），其他逐步添加

---

**创建日期**: 2026-07-22
