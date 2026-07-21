# Stage 4.10 开发计划：宏系统基础

> **阶段**: Stage 4.10
> **版本**: v0.9.6 → v0.9.7
> **状态**: 🔄 In progress
> **流程**: stage-committee-process.md v3.17 §17.3 时期 1

## 1. 目标

实现基本宏展开系统：在 HIR lowering 之前展开已知的内置宏，
使 `println!(...)`、`format!(...)` 等宏调用产生合理的 MIR 而非 `TyKind::Error`。

## 2. 背景

当前 `HirExprKind::MacroCall` 在 MIR lowering 中产生 `TyKind::Error` placeholder。
Stage 4.10 实现：
- `MacroExpander` 结构 — 在 driver 流水线中 lex→parse→**expand**→lower→resolve→mir
- 内置宏注册表 — `println!`、`stringify!`、`assert!`
- 宏展开将 `MacroCall` 替换为展开后的 `HirExpr`

## 3. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 4.10-a | 创建 `src/macro_expand/mod.rs` — MacroExpander + 内置宏注册表 | L2 |
| 4.10-b | 实现 `println!` → `Call(printf_placeholder)` 展开简化 | L2 |
| 4.10-c | 实现 `stringify!` → `Lit(Str)` 展开 | L1 |
| 4.10-d | 在 driver 中插入 expand 步骤 | L1 |
| 4.10-e | 添加测试 | L1 |

## 4. 简化方案

完整的 `macro_rules!` 用户自定义宏系统非常复杂（需要 token tree 匹配 + 重写）。
Stage 4.10 采用简化方案：
- 只支持内置宏（`println!`、`stringify!`、`assert!`）
- 宏展开在 AST→HIR lowering 阶段进行（在 HIR lower 中处理）
- `println!(...)` → 简化为 unit 表达式（不实际打印）
- `stringify!(expr)` → `Lit(Str(...))` 字符串字面量

## 5. 验收标准

1. `cargo build` 0 warnings
2. `cargo clippy --all-targets -- -D warnings` 通过
3. `cargo fmt --check` 通过
4. 至少 3 个新测试
5. §17.3 三阶段文档协议执行

---

**创建日期**: 2026-07-22
