# Stage 18.59 — LowerTyCtx Consolidation + typeck make_mismatch Span

> **Author**: redskaber + ARCH-A + DEV-A + QA-A
> **Date**: 2026-08-08
> **Version**: v0.325.0 → v0.326.0
> **Process**: stage-committee-process.md v5.0 §13.1 + §13.5 + §14 (deep review)
> **Status**: ✅ Design Complete — Ready for Implementation

---

## 1. 背景 (§13.1 阶段开始设计对齐)

### 1.1 审计发现 (Stage 18.56)

`lower_hir_ty_to_mir_ty*` 有 7 个变体 (参数组合反模式):
1. `lower_hir_ty_to_mir_ty(ty)` — 基础
2. `lower_hir_ty_to_mir_ty_with_lifetimes(ty, region_counter, lifetime_map)` — 独立实现
3. `lower_hir_ty_to_mir_ty_with_hir(ty, hir)` — thin wrapper
4. `lower_hir_ty_to_mir_ty_with_regions(ty, region_counter)` — thin wrapper
5. `lower_hir_ty_to_mir_ty_with_regions_and_hir(ty, region_counter, hir)` — 主实现
6. `lower_hir_ty_to_mir_ty_with_generics(ty, generic_params)` — thin wrapper
7. `lower_hir_ty_to_mir_ty_with_generics_and_regions(ty, generic_params, region_counter)` — thin wrapper

审计建议: "引入 `LowerTyCtx { region_counter, hir, generic_params, errors }` 并将 7 个变体合并为一个。"

### 1.2 本阶段目标

引入 `LowerTyCtx` struct, 将 7 个变体合并为 1 个入口 + context。同时处理 Priority 5 (typeck `make_mismatch` span)。

**做**:
- 新增 `LowerTyCtx` struct: `{ region_counter, hir, generic_params }`
- 新增 `lower_hir_ty_to_mir_ty_with_ctx(ty, cx) -> Ty` — 单一入口
- 旧 7 个变体保留为 deprecated thin wrappers (向后兼容)
- typeck `make_mismatch`: 文档化 create-then-overwrite 模式 (Priority 5 推迟原因)

**不做** (留待后续):
- ❌ 立即迁移所有调用者 (渐进迁移, 避免大范围破坏)
- ❌ `lower_hir_ty_to_mir_ty_with_lifetimes` 合并 (独立实现, 不易迁移)
- ❌ `Const` struct 添加 span (Priority 6, 远期)

### 1.3 设计原则遵循

| 原则 | 如何遵循 |
|------|---------|
| 2. 整体 > 局部 | 一个 context struct 处理所有参数组合 |
| 5. 去除兼容思维 | 7 变体 → 1 入口 + context |
| 6. 通用 > 特例 | LowerTyCtx 一个 struct 处理所有 context |
| 7. API 命名标准化 | `LowerTyCtx` / `lower_hir_ty_to_mir_ty_with_ctx` |

---

## 2. 技术设计

### 2.1 LowerTyCtx struct (src/mir/lower/mod.rs)

```rust
/// Stage 18.59: Lowering context for HIR→MIR type lowering.
///
/// Replaces 7 `lower_hir_ty_to_mir_ty*` variant functions with a single
/// context struct + entry point.
///
/// Per §1.0 原則 6 "通用 > 特例": one context handles all combinations.
/// Per §1.0 原則 5 "去除兼容思维": replaces parameter-combination anti-pattern.
pub(crate) struct LowerTyCtx<'a> {
    pub region_counter: &'a mut u32,
    pub hir: Option<&'a HirCrate>,
    pub generic_params: &'a [crate::mir::ty::ParamTy],
}

impl<'a> LowerTyCtx<'a> {
    /// Create a context with all fields set to defaults (no hir, no generics).
    pub fn new(region_counter: &'a mut u32) -> Self {
        Self {
            region_counter,
            hir: None,
            generic_params: &[],
        }
    }

    /// Builder: set hir.
    pub fn with_hir(mut self, hir: Option<&'a HirCrate>) -> Self {
        self.hir = hir;
        self
    }

    /// Builder: set generic_params.
    pub fn with_generics(mut self, generic_params: &'a [crate::mir::ty::ParamTy]) -> Self {
        self.generic_params = generic_params;
        self
    }
}
```

### 2.2 新入口: lower_hir_ty_to_mir_ty_with_ctx

```rust
/// Stage 18.59: Single entry point for HIR→MIR type lowering.
///
/// Replaces 7 `lower_hir_ty_to_mir_ty*` variants. Callers construct a
/// `LowerTyCtx` and pass it here.
///
/// Per §10 naming: `lower_hir_ty_to_mir_ty_with_ctx` follows
/// `<verb>_<noun>_<prep>_<noun>` pattern.
pub(crate) fn lower_hir_ty_to_mir_ty_with_ctx(ty: &HirTy, cx: &mut LowerTyCtx) -> Ty {
    // Delegate to the existing main implementation, passing context fields.
    // If generic_params is non-empty, use the generics-aware path.
    if !cx.generic_params.is_empty() {
        lower_hir_ty_to_mir_ty_with_generics_and_regions_impl(
            ty,
            cx.generic_params,
            cx.region_counter,
        )
    } else {
        lower_hir_ty_to_mir_ty_with_regions_and_hir(
            ty,
            cx.region_counter,
            cx.hir,
        )
    }
}
```

### 2.3 旧变体保留为 deprecated wrappers

```rust
#[deprecated(note = "Stage 18.59: use lower_hir_ty_to_mir_ty_with_ctx + LowerTyCtx")]
pub(crate) fn lower_hir_ty_to_mir_ty(ty: &HirTy) -> Ty { ... }

#[deprecated(note = "Stage 18.59: use lower_hir_ty_to_mir_ty_with_ctx + LowerTyCtx")]
pub(crate) fn lower_hir_ty_to_mir_ty_with_hir(ty: &HirTy, hir: Option<&HirCrate>) -> Ty { ... }
// ... etc for all 7 variants
```

---

## 3. §6.3 委员会投票 (模拟)

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | 反模式清理, 高内聚低耦合 |
| DEV-A | GO | 渐进迁移, 旧变体保留 |
| QA-A | GO | 可测试 LowerTyCtx builder |
| REV-A | GO | 审计驱动, 设计原则遵循 |
| PM-A | GO | 技术债清理 |

**5/5 GO** ✅
