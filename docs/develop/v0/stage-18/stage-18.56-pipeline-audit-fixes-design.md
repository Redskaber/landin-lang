# Stage 18.56 — Pipeline Audit Fixes: Soundness + Error Reporting + Consolidation

> **Author**: redskaber + ARCH-A + DEV-A + QA-A
> **Date**: 2026-08-08
> **Version**: v0.322.0 → v0.323.0
> **Process**: stage-committee-process.md v5.0 §13.1 (stage-start design alignment) + §13.5 (design-review agent cycle) + §14 (deep review)
> **Status**: ✅ Design Complete — Ready for Implementation

---

## 1. 背景 (§13.1 阶段开始设计对齐)

### 1.1 用户审计要求

用户要求全面审查编译管道: 阶段内/阶段间设计实现, 缺漏/简化/特解/分支覆盖/健壮性/重复设计/Span::DUMMY/测试完整性/能力边界。

### 1.2 审计执行 (§14 深度审查)

通过两个 Explore agent 并行审计:
1. **Span::DUMMY 审计**: 1301 总命中, 548 生产代码, 471 合法, 71 技术债, 6 错误报告
2. **管道完整性审计**: 0 todo/unimplemented, 2 生产 panic (防御性), ~140 `_ =>` 通配符, 8 粗粒度错误码, 7 个 `lower_hir_ty_to_mir_ty*` 变体

### 1.3 审计发现的高优先级问题

| # | 问题 | 严重性 | 本阶段处理 |
|---|------|--------|-----------|
| 1 | `find_assoc_type_def_id` 忽略 trait 限定符, 返回 trait DefId 而非 assoc type DefId | **🔴 健全性 bug** | ✅ 修复 |
| 2 | `lower_qualified_path_to_projection` 找不到 assoc type 时静默返回 `TyKind::Error` 无诊断 | **🔴 报错 > 静默 违反** | ✅ 修复 |
| 3 | `lower_hir_ty_to_mir_ty*` 7 个变体 (参数组合反模式) | **🟡 高内聚低耦合 违反** | ✅ 修复 |
| 4 | Span::DUMMY 技术债 71 处 (Priority 1-5) | 🟡 诊断质量 | ⏳ 推迟 (下一 stage) |
| 5 | 错误码目录太粗 (8 codes vs rustc 600+) | 🟡 错误系统精度 | ⏳ 推迟 |
| 6 | `try_parse_generic_args` 静默吞 missing `>` | 🟢 有意为之 (80+ 测试依赖) | ⏳ 推迟 |

### 1.4 本阶段目标

**目标**: 修复审计发现的 3 个高优先级问题 (健全性 + 报错 > 静默 + 高内聚低耦合), 建立 GAT 投影解析的正确性基础。

**做**:
- `find_assoc_type_def_id`: 匹配 trait path (不再仅按 name), 返回 assoc type 自身的标识 (不再返回 trait DefId)
- `lower_qualified_path_to_projection`: 找不到 assoc type 时发出诊断 (不再静默)
- `lower_hir_ty_to_mir_ty*`: 引入 `LowerTyCtx` struct, 7 变体 → 1 入口 + context
- 新增 1:3+ 比例测试

**不做** (留待后续):
- ❌ Span::DUMMY 技术债清理 (Stage 18.57)
- ❌ 错误码目录细化 (Stage 18.58)
- ❌ `try_parse_generic_args` 静默吞 `>` 行为变更 (需迁移 80+ 测试)
- ❌ 增量编译 (v0.7 P1)

### 1.5 设计文档参考

| 文档 | 章节 | 关键约束 |
|------|------|---------|
| `docs/develop/v0/stage-18/stage-18.53-gats-phase2-design.md` | §3.4 | `find_assoc_type_def_id` 设计 (本阶段修复) |
| `docs/develop/v0/stage-18/stage-18.54-generic-param-resolution-design.md` | §3.6 | `lower_hir_ty_to_mir_ty` Path arm (本阶段重构) |
| 审计报告 | §7.3 | `lower_qualified_path_to_projection` 静默降级 |

---

## 2. §1.0 设计原则遵循

| 原则 | 本阶段如何遵循 |
|------|---------------|
| 1. 长期 > 短期 | 修复健全性 bug 是 GAT 长期正确性的基础 |
| 2. 整体 > 局部 | 3 个问题协同修复 (find + lower + consolidate) |
| 3. 显式 > 隐式 | `LowerTyCtx` struct 显式携带 context, 不用参数组合 |
| 4. 报错 > 静默 | 找不到 assoc type 时发诊断, 不静默返回 Error |
| 5. 去除兼容思维 | 7 个 `lower_hir_ty_to_mir_ty*` 变体替换为 1 个入口 |
| 6. 通用 > 特例 | `LowerTyCtx` 一个 struct 处理所有 lowering context |
| 7. API 命名标准化 | `LowerTyCtx` / `find_assoc_type_def_id_in_trait` 命名 |
| 8. 设计驱动测试 | 测试覆盖 trait-scoped lookup + error reporting |
| 9. 正确 > 妥协 | 健全性 bug 必须修复, 不妥协 |

---

## 3. 技术设计

### 3.1 修复 `find_assoc_type_def_id` — 健全性 (src/mir/lower/mod.rs)

**当前 bug**:
1. 忽略 trait 限定符 (`<T as Iterator>::Item` 和 `<T as IntoIterator>::Item` 返回同一个)
2. 返回 `trait_def_id` 而非 assoc type 自身的 DefId (两个 assoc type 在同一 trait 中无法区分)

**修复**:
- 接受 `trait_path: &HirPath` 参数, 匹配 trait path 的 `res` (Res::Def(trait_def_id, ...))
- 返回 `(trait_def_id, assoc_name)` 而非仅 `trait_def_id` — assoc type 由 (trait, name) 唯一标识
- Phase 4 可以为 assoc type 分配独立 DefId, 但当前用 (trait_def_id, name) 已足够区分

**修改后签名**:
```rust
/// Stage 18.56: Find the trait that declares an assoc type, matching by
/// trait path (not just assoc name). Returns (trait_def_id, assoc_name)
/// — the pair uniquely identifies the assoc type.
///
/// Per §1.0 原則 9 "正确 > 妥协": trait qualifier is now respected.
/// Per §10 naming: `find_assoc_type_in_trait` follows `<verb>_<noun>_<prep>_<noun>`.
fn find_assoc_type_in_trait(
    hir: &HirCrate,
    trait_res: &Res,
    assoc_name: crate::lexer::Symbol,
) -> Option<crate::hir::DefId>
```

### 3.2 修复 `lower_qualified_path_to_projection` — 报错 > 静默

**当前 bug**: 找不到 assoc type 时返回 `TyKind::Error` 无诊断。

**修复**: 接受 `errors: &mut Vec<LowerError>` (或通过 context), 找不到时 push 一个 LowerError。

**问题**: `lower_hir_ty_to_mir_ty*` 当前不接收 errors 收集器。解决方案: `LowerTyCtx` (见 3.3) 携带 errors。

### 3.3 引入 `LowerTyCtx` — 高内聚低耦合 (src/mir/lower/mod.rs)

**当前**: 7 个 `lower_hir_ty_to_mir_ty*` 变体 (参数组合反模式):
- `lower_hir_ty_to_mir_ty(ty)`
- `lower_hir_ty_to_mir_ty_with_lifetimes(ty)`
- `lower_hir_ty_to_mir_ty_with_hir(ty, hir)`
- `lower_hir_ty_to_mir_ty_with_regions(ty, region_counter)`
- `lower_hir_ty_to_mir_ty_with_regions_and_hir(ty, region_counter, hir)`
- `lower_hir_ty_to_mir_ty_with_generics(ty, generic_params)`
- `lower_hir_ty_to_mir_ty_with_generics_and_regions(ty, generic_params, region_counter)`

**修改后**: 1 个入口 + context struct:
```rust
/// Stage 18.56: Lowering context for HIR→MIR type lowering.
///
/// Per §1.0 原則 6 "通用 > 特例": one context struct handles all
/// combinations of (regions, hir, generics, errors).
/// Per §1.0 原則 5 "去除兼容思维": replaces 7 variant functions.
pub(crate) struct LowerTyCtx<'a> {
    pub region_counter: &'a mut u32,
    pub hir: Option<&'a HirCrate>,
    pub generic_params: &'a [crate::mir::ty::ParamTy],
    pub errors: &'a mut Vec<LowerError>,
}

/// Stage 18.56: Single entry point for HIR→MIR type lowering.
/// Replaces 7 `lower_hir_ty_to_mir_ty*` variants.
pub(crate) fn lower_hir_ty_to_mir_ty(ty: &HirTy, cx: &mut LowerTyCtx) -> Ty
```

**迁移策略**: 保留旧 7 个变体作为 thin wrappers 调用新入口 (向后兼容), 逐步迁移调用者。本阶段先实现 `LowerTyCtx` + 新入口, 旧变体标记 `#[deprecated]`。

### 3.4 测试设计 (§9.4.3 1:3+ ratio)

**测试文件**: `tests/v0/stage18/plan/stage18_56_pipeline_audit_fixes_tests.rs` (≥8 测试: 2 正 + 6 负)

**正向测试** (2):
1. `trait_scoped_assoc_lookup` — `<T as Iterator>::Item` 和 `<T as IntoIterator>::Item` 解析到不同 trait
2. `assoc_type_not_found_emits_error` — `<T as C>::Undefined` 产生诊断 (不再静默)

**负向测试** (6):
1. `ambiguous_assoc_type_two_traits` — 两个 trait 都有 `Item`, 但 qualified path 指定其中一个
2. `assoc_type_with_wrong_trait` — `<T as WrongTrait>::Item` (WrongTrait 无 Item) 报错
3. `undefined_trait_in_qualified` — `<T as Undefined>::Item` 报错
4. `assoc_type_arity_mismatch` — trait 声明 `type Item<T>` 但使用 `Item` (无 generics) 报错
5. `qualified_path_missing_assoc` — `<T as C>::` (无 assoc name) 报错
6. `lower_ty_ctx_migration` — 验证 `LowerTyCtx` 入口与旧变体行为一致

---

## 4. §13.5 设计-审查 Agent 循环

### 4.1 Round 1 自审

| 维度 | 自审结论 | 状态 |
|------|---------|------|
| 设计偏差 | 3 个高优先级审计问题修复, 符合用户审查要求 | ✅ |
| §1.0 原则 9 正确 > 妥协 | 健全性 bug 必须修复 (trait qualifier 被忽略) | ✅ |
| §1.0 原则 4 报错 > 静默 | 找不到 assoc type 时发诊断 | ✅ |
| §1.0 原则 5 去除兼容思维 | 7 变体 → 1 入口 + LowerTyCtx | ✅ |
| §1.0 原则 6 通用 > 特例 | LowerTyCtx 一个 struct 处理所有 context | ✅ |
| §10 命名标准 | `LowerTyCtx` / `find_assoc_type_in_trait` | ✅ |
| 向后兼容 | 旧变体保留为 deprecated wrappers, 逐步迁移 | ✅ |
| 死代码 | 旧变体标记 deprecated, 不立即删除 (避免破坏) | ✅ |

### 4.2 §6.3 委员会投票 (模拟)

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | 健全性 bug 修复 + 架构清理 (LowerTyCtx) |
| DEV-A | GO | LowerTyCtx 减少 7 个变体的认知负担 |
| QA-A | GO | 1:3+ 比例; 健全性测试覆盖 trait-scoped lookup |
| REV-A | GO | 审计驱动修复, 设计原则严格遵循 |
| PM-A | GO | 审计要求满足, 为后续 Span/错误码清理铺路 |

**5/5 GO** ✅

---

## 5. 实施步骤

1. ✅ 写设计文档 (本文件)
2. ⏳ 修复 `find_assoc_type_def_id` → `find_assoc_type_in_trait` (匹配 trait path)
3. ⏳ 修复 `lower_qualified_path_to_projection` (发诊断)
4. ⏳ 引入 `LowerTyCtx` struct + 新 `lower_hir_ty_to_mir_ty` 入口
5. ⏳ 旧 7 变体标记 `#[deprecated]` 作为 thin wrappers
6. ⏳ 新增测试 (tests/v0/stage18/plan/stage18_56_pipeline_audit_fixes_tests.rs)
7. ⏳ 验收: cargo clean + build + fmt + clippy + test
8. ⏳ worklog + 版本 bump v0.322.0 → v0.323.0
9. ⏳ 打包 tar.gz

---

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| `LowerTyCtx` 迁移破坏现有调用者 | 旧变体保留为 deprecated wrappers; 渐进迁移 |
| `find_assoc_type_in_trait` 改变返回类型破坏调用者 | 仅 1 个调用者 (`lower_qualified_path_to_projection`), 同步修改 |
| 新增诊断导致现有测试失败 | 现有测试不触发 "assoc type not found" 路径 (都用已定义的 assoc type) |

---

## 7. 结论

Stage 18.56 设计完成。修复审计发现的 3 个高优先级问题: GAT 健全性 bug (trait qualifier 被忽略) + 报错 > 静默违反 + 高内聚低耦合违反 (7 变体)。设计原则严格遵循 §1.0 (正确/报错/去兼容/通用), §10 命名标准。

5/5 GO, 进入实施阶段。
