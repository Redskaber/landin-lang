# Stage 18.57 — Span::DUMMY Technical Debt Cleanup (Priority 1-5)

> **Author**: redskaber + ARCH-A + DEV-A + QA-A
> **Date**: 2026-08-08
> **Version**: v0.323.0 → v0.324.0
> **Process**: stage-committee-process.md v5.0 §13.1 + §13.5 + §14 (deep review)
> **Status**: ✅ Design Complete — Ready for Implementation

---

## 1. 背景 (§13.1 阶段开始设计对齐)

### 1.1 上一阶段审计 (Stage 18.56)

Span::DUMMY 审计发现 548 生产代码命中:
- 471 LEGITIMATE (合成类型, 无源位置 — 不改)
- 71 TECHNICAL DEBT (真实 span 存在但被丢弃 — 本阶段修复)
- 6 ERROR REPORTING (诊断 span 不准确 — 本阶段修复)

### 1.2 本阶段目标

修复审计 Priority 1-5 的 TECHNICAL DEBT + ERROR REPORTING 站点 (~50 处), 提升诊断 span 准确性。

**做**:
- Priority 1: `lower_hir_ty_to_mir_ty*` — `let span = ty.span` 替代 `Span::DUMMY` (3 函数, ~30 hits)
- Priority 2: `hir/lower/pat.rs` `pat_span()` — 使用 ident/literal span
- Priority 3: `hir/lower/item.rs` `HirAssocType`/`HirAssocConst` — 使用 ident.span
- Priority 4: `resolve/module_build.rs` duplicate-definition errors — 使用 def span map
- Priority 5: `typeck/unify.rs` `make_mismatch` — 传入 span 参数

**不做** (留待后续):
- ❌ LEGITIMATE 站点 (471 处, 合成类型, 不改)
- ❌ `Const` struct 添加 span 字段 (Priority 6, 需大范围修改)
- ❌ `LowerTyCtx` 合并 (Priority 7, Stage 18.59)

### 1.3 设计原则遵循

| 原则 | 如何遵循 |
|------|---------|
| 3. 显式 > 隐式 | span 显式从 HIR 传递到 MIR |
| 4. 报错 > 静默 | 诊断 span 准确, 不用 DUMMY 占位 |
| 6. 通用 > 特例 | 一个 `ty.span` 修复覆盖 3 个函数 |
| 7. API 命名标准化 | `make_mismatch` 显式 span 参数 |

---

## 2. 实施计划

### Priority 1: `lower_hir_ty_to_mir_ty*` (src/mir/lower/mod.rs)

3 个函数的 `let span = Span::DUMMY;` 改为 `let span = ty.span;`:
- `lower_hir_ty_to_mir_ty_with_regions_and_hir` (line 1867)
- `lower_hir_ty_to_mir_ty_with_lifetimes` (line 1577)
- `lower_hir_ty_to_mir_ty_with_generics_and_regions` (line 2160)

### Priority 2: `hir/lower/pat.rs` `pat_span()`

返回 ident.span / literal span 而非 DUMMY。

### Priority 3: `hir/lower/item.rs` `HirAssocType`/`HirAssocConst`

使用 ident.span 设置 span 字段。

### Priority 4: `resolve/module_build.rs` duplicate-definition

使用 def_span_map 查找 span。

### Priority 5: `typeck/unify.rs` `make_mismatch`

`make_mismatch` 接受 `span: Span` 参数, 调用者传入 `stmt.span`。

---

## 3. §6.3 委员会投票 (模拟)

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | 诊断质量提升, 风险低 |
| DEV-A | GO | 机械修复, 1-line per site |
| QA-A | GO | 可测试 span 准确性 |
| REV-A | GO | 审计驱动, 优先级清晰 |
| PM-A | GO | 技术债清理 |

**5/5 GO** ✅
