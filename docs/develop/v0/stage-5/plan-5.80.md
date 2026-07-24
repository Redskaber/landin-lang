# Stage 5.80 开发计划：driver 集成 — 自动构建 DynTraitMIRPlan 并传入 lower

> **阶段**: Stage 5.80
> **版本**: v0.11.75 → v0.11.76
> **状态**: ✅ Complete

## 1. 目标

在 driver 中自动构建 `DynTraitMIRPlan`（via
`build_dyn_trait_mir_plan_from_resolver(&trait_resolver, &interner)`），
并通过新的 lower 入口点 `lower_hir_body_to_mir_full_with_dyn_trait_plan()`
传入每个 body 的 lowering context。这激活了 Stage 5.78 的
`HirExprKind::MethodCall` dyn Trait 路径 + Stage 5.79 的 codegen
vtable indirect call，形成**端到端 dyn Trait 编译管线**。

## 2. 设计动机

Stage 5.78 + 5.79 完成了 dyn Trait lowering → codegen pipeline，但
**只在测试中**通过 `cx.set_dyn_trait_plan()` 手动接线。生产 driver
流程仍未激活 dyn Trait——所有 MethodCall 仍走 Error placeholder 路径。

Stage 5.80 在 driver 中自动构建 plan 并传入 lower，让正常编译流程
真正使用 dyn Trait 路径。这标志着 **dyn Trait MIR lowering 正式
接入主管线**。

## 3. 设计

### 3.1 新增 lower 入口点

```rust
/// Stage 5.80: Full lowering entry point with optional DynTraitMIRPlan.
///
/// When `plan` is `Some`, attaches it to the MirLowerCtxt via
/// `cx.set_dyn_trait_plan(plan.clone())` — this activates the
/// HirExprKind::MethodCall dyn Trait path (Stage 5.78). The clone
/// happens once per body (acceptable cost; plan is small).
///
/// When `plan` is `None`, behavior is identical to
/// `lower_hir_body_to_mir_full` (legacy path).
pub fn lower_hir_body_to_mir_full_with_dyn_trait_plan(
    body: &Body,
    interner: &Rodeo,
    hir: &HirCrate,
    return_ty: Option<HirTy>,
    plan: Option<&DynTraitMIRPlan>,
) -> (MirBody, UnificationTable)
```

### 3.2 重构现有入口点

`lower_hir_body_to_mir_full` 变为薄包装，委托给新函数（plan = None）。
保持向后兼容——所有现有调用点不变。

### 3.3 Driver 修改

在 body 循环之前构建一次 plan：

```rust
// Stage 5.80: build DynTraitMIRPlan once for the whole crate.
// Per §16: driver is the orchestrator that connects TraitResolver
// (Stage 5.2) to mir::lower (Stage 2.1) via the plan data structure.
let dyn_trait_plan = build_dyn_trait_mir_plan_from_resolver(
    &trait_resolver, &interner);

for (body_id, body) in &hir.bodies {
    let return_ty = hir.owner(body_id.owner.0).and_then(owner_return_ty);
    let (mut mir, lower_unify) =
        lower_hir_body_to_mir_full_with_dyn_trait_plan(
            body, &interner, &hir, return_ty, Some(&dyn_trait_plan));
    // ... existing typeck + borrowck ...
}
```

### 3.4 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `lower_hir_body_to_mir_full_with_dyn_trait_plan` | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>_<noun>_<noun>` | ✅ |

参考 §2.2 canonical entry points 表 + Rust API guidelines 的
`_with_<feature>` 后缀约定（如 `Vec::with_capacity`、
`HashMap::with_hasher`）。新函数是 `_full` 变体的扩展版本。

### 3.5 §16 接口隔离

- 输入：`&Body` + `&Rodeo` + `&HirCrate` + `Option<HirTy>` + `Option<&DynTraitMIRPlan>`
- 输出：`(MirBody, UnificationTable)`
- 数据流：driver → `build_dyn_trait_mir_plan_from_resolver` →
  `lower_hir_body_to_mir_full_with_dyn_trait_plan` →
  `cx.set_dyn_trait_plan` → `lower_expr_to_operand` (Stage 5.78) →
  `codegen_dyn_trait_call` (Stage 5.79)
- 单向，无循环依赖

### 3.6 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | 新入口点 plan=None 等价于 lower_hir_body_to_mir_full | MIR 一致 |
| 2 | 新入口点 plan=Some 后 cx.dyn_trait_plan 是 Some | ✓ |
| 3 | 新入口点 plan=Some(empty plan) 不影响 MIR | MIR 一致 |
| 4 | 新入口点 plan=Some(non-empty) 当 MethodCall 匹配时走 dyn Trait | side-table 非空 |
| 5 | 现有 lower_hir_body_to_mir_full 行为不变 | 现有测试通过 |
| 6 | driver 集成：编译含 dyn Trait 的源码时 plan 自动构建 | ✓ |
| 7 | driver 集成：无 dyn Trait 时 plan 为空 | side-table 为空 |
| 8 | driver 集成：现有所有测试不变 | 现有测试通过 |
| 9 | 端到端：源码 → driver → MIR 含 dyn_trait_calls | ✓ |
| 10 | plan 与 trait_resolver.vtables 内容一致 | ✓ |

## 4. 不在本 stage 范围

- ❌ codegen 实际生成 vtable indirect call IR 的端到端测试
  （已在 5.79 测试，本 stage 关注 driver 接线）
- ❌ dyn Trait return type 的精确处理（仍用 I32 placeholder）
- ❌ driver 中显式 vtable/dynptr 全局发射（已在 Stage 5.7-5.60 完成）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
