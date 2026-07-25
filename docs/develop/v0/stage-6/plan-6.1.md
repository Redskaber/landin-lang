# Stage 6.1 开发计划：mir/lower/mod.rs 拆分 — ADT layout 提取（TD-011 第一步）

> **阶段**: Stage 6.1（Stage 6 第一个子阶段）
> **版本**: v0.11.95 → v0.12.0
> **状态**: ✅ Complete

## 1. 目标

开始偿还 TD-011（mir/lower/mod.rs 3346 LOC 拆分）。第一步：将 ADT layout
相关函数（~140 LOC）从 `mir/lower/mod.rs` 提取到独立模块 `mir/lower/adt_layout.rs`。

## 2. 拆分计划

### 2.1 提取的函数

| 函数 | LOC | 职责 |
|------|-----|------|
| `populate_adt_layouts` | 50 | 遍历 MirBody，收集 DefIds，注册 layouts |
| `collect_adt_def_ids` | 15 | 遍历 Ty，收集 Adt DefIds |
| `build_adt_layout` | 40 | 从 HIR 构建 AdtLayout |
| `AdtLayoutExt` trait + impl | 28 | 嵌套 DefId 提取扩展方法 |

### 2.2 依赖处理

- `build_adt_layout` 调用 `lower_hir_ty_to_mir_ty`（在 mod.rs 中定义）
  → 将 `lower_hir_ty_to_mir_ty` 改为 `pub(crate)`
- 其他依赖（MirBody, HirCrate, Ty 等）通过 `use` 导入

### 2.3 §16 接口隔离

提取后 `mir/lower/adt_layout.rs` 依赖：
- `mir::body::*`（MirBody, AdtLayout, StatementKind）
- `mir::place::*`（Rvalue, AggregateKind）
- `mir::ty::*`（Ty, TyKind, Span）
- `hir::*`（HirCrate, DefId, OwnerNode, HirItem）
- `mir::lower::lower_hir_ty_to_mir_ty`（pub(crate)）

所有依赖都是单向（mir::lower 内部模块间），无循环。✅

### 2.4 命名标准化

无新公共 API——仅内部模块重组。`populate_adt_layouts` 从 `fn` 改为 `pub(crate) fn`。

### 2.5 测试

现有 1881 个测试全部通过不变（行为等价重构）。无需新测试。

---

**创建日期**: 2026-07-24
