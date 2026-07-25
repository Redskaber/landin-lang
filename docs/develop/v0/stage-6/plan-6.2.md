# Stage 6.2 开发计划：mir/lower/mod.rs 拆分 — closure capture 提取（TD-011 第二步）

> **阶段**: Stage 6.2
> **版本**: v0.12.0 → v0.12.1
> **状态**: ✅ Complete

## 1. 目标

继续偿还 TD-011。第二步：将 closure capture 相关函数（~163 LOC）从
`mir/lower/mod.rs` 提取到独立模块 `mir/lower/closure_capture.rs`。

## 2. 提取的函数

| 函数 | LOC | 职责 |
|------|-----|------|
| `collect_captured_locals` | ~137 | 遍历 HirExpr，收集闭包捕获的外部局部变量 |
| `collect_block_captured` | ~26 | 遍历 HirBlock，收集捕获的变量 |

## 3. 依赖处理

- 依赖 `MirLowerCtxt`（通过 `&MirLowerCtxt` 引用）
- 依赖 `HirExpr`, `HirExprKind`, `HirBlock`, `HirStmt`, `Res`, `HirId`（来自 `hir::*`）
- 依赖 `LocalId`（来自 `mir::place::*`）
- `MirLowerCtxt` 需要 `pub(crate)` 可见性（当前已是 `pub struct`）

---

**创建日期**: 2026-07-24
