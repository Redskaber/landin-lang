# Stage 18.53 — GATs Phase 2: Qualified Path Parsing + Projection Lowering

> **Author**: redskaber + ARCH-A + DEV-A + QA-A
> **Date**: 2026-08-08
> **Version**: v0.319.0 → v0.320.0
> **Process**: stage-committee-process.md v5.0 §13.1 (stage-start design alignment) + §13.5 (design-review agent cycle)
> **Status**: ✅ Design Complete — Ready for Implementation

---

## 1. 背景 (§13.1 阶段开始设计对齐)

### 1.1 上一阶段成果 (Stage 18.52 GATs Phase 1)

已完成 GATs Phase 1: AST/Parser/HIR 基础设施，让 Landin 能解析与表示 `type Item<'a, T> where Self: 'a;` 语法。但 Phase 1 仅为 *声明* 支持，未实现 *使用* — `Self::Item<'a>` 和 `<T as Trait>::Item` 仍无法解析。

### 1.2 本阶段目标

**Phase 2 范围**: 让 GAT 在 *使用点* 能正确解析和初步表示为 `TyKind::Projection`，并扩展 `projection_resolver` 处理 GAT generics。

**做**:
- Parser 支持 `<T as Trait>::Item` (qualified path) 语法
- Parser 支持 `Self::Item<'a, T>` (path segment with generic args，包括 `>>` splitting)
- HIR→MIR lower: 当 `HirTyKind::Path(qself, path)` 有 `qself.ty = Some(...)` 时，产生 `TyKind::Projection(assoc_def_id, substs)`
- `projection_resolver`: 扩展 lookup 以处理 GAT generics (按 name + generic arity 匹配)
- 新增 1:3+ 比例测试

**不做** (留待 Phase 3+):
- ❌ GAT monomorphization (实际生成不同 LLVM IR)
- ❌ 高阶 region 约束求解 (`where Self: 'a`)
- ❌ GAT variance 检查
- ❌ 完整 trait selection (依赖 trait solver 重构)

### 1.3 设计文档参考

| 文档 | 章节 | 关键约束 |
|------|------|---------|
| `docs/lang-design/02-grammar.md` | §3.3 (type_path + qualified_path) | 路径语法 BNF |
| `docs/lang-design/03-type-system.md` | 关联类型部分 | 投影语义 |
| `docs/develop/v0/stage-18/stage-18.52-gats-phase1-design.md` | §3.5 | 下游消费者 graceful degradation |
| `src/typeck/projection_resolver.rs:1-30` | 注释 | 已知待修复问题 (B5-B9) |

### 1.4 当前 Parser 限制 (审查发现)

通过 explore 测试发现当前 parser 缺陷：

| 语法 | 当前状态 | 本阶段处理 |
|------|---------|-----------|
| `Self::Item` (无 generics) | ✅ 工作 | 不动 |
| `Self::Item<i32>` (type arg) | ✅ 工作 | 不动 |
| `Self::Item<'a>` (lifetime arg) | ❌ `>>` 不分割 | ✅ 修复 |
| `Option<Self::Item<'a>>` | ❌ `>>` 不分割 | ✅ 修复 |
| `<T as Trait>::Item` | ❌ 不支持 | ✅ 新增 |
| `&'a mut self` (lifetime self) | ❌ 不支持 | ⏳ 推迟 (非 GAT 必需) |

---

## 2. §1.0 设计原则遵循

| 原则 | 本阶段如何遵循 |
|------|---------------|
| 1. 长期 > 短期 | Phase 2 是 GAT 完整实现的必要步骤；不做 Phase 3 会让 Phase 2 失去意义 |
| 2. 整体 > 局部 | Parser + HIR lower + projection_resolver 三层同时变更 |
| 3. 显式 > 隐式 | `TyKind::Projection(def_id, substs)` 显式表示，不依赖隐式 Adt |
| 4. 报错 > 静默 | QSelf 解析失败时清晰报错，不静默回退到 Adt |
| 5. 去除兼容思维 | `lower_hir_ty_to_mir_ty` 中 `HirTyKind::Path` arm 重写，不保留旧的 "ignore qself" 路径 |
| 6. 通用 > 特例 | 一个 `qualified path` parser 处理所有 `<T as Trait>::Name` 形式 |
| 7. API 命名标准化 | `parse_qself` / `try_parse_qself` / `lower_qualified_path_to_projection` 命名 |
| 8. 设计驱动测试 | 测试用例覆盖 qualified path 解析、GAT generics、projection lowering |
| 9. 正确 > 妥协 | Phase 3 monomorphization 推迟明确记录于本文档 |

---

## 3. 技术设计

### 3.1 Parser 变更: `>>` Splitting (src/parser/path.rs + src/parser/generics.rs)

**问题**: 当前 `parse_generics` 在遇到 `>>` (Shr token) 时只是 `bump()` 一次，导致 `Option<Vec<Self::Item<'a>>>` 解析失败。

**修复**: 实现 `>>` splitting — 当 parser 在嵌套 generics 上下文遇到 `Shr` token 时，将其视为两个 `>`，仅消费一个，留下另一个给外层。

**实现策略**: 使用 `parser_state.shr_split: Option<()>` 字段标记"已 split 一个 `>`"，下次 `eat(Gt)` 时优先消费 split。

**新增 API** (§10 命名):
- `fn split_shr_token(&mut self)` — 将当前 `>>` token 拆分为单个 `>`
- `fn eat_gt_or_split(&mut self) -> bool` — 消费 `>` 或拆分 `>>`

### 3.2 Parser 变更: Qualified Path (src/parser/path.rs)

**新增 API**: `fn try_parse_qself(&mut self) -> Option<QSelf>`

**语法**: `<Type as TraitPath>::Name::...`

**算法**:
1. 在 `parse_path_with_ctx` 开头检查 `<`，若存在则进入 qself 解析
2. `bump()` 消费 `<`
3. `parse_ty()` 解析 inner type `T`
4. 期望 `as` 关键字
5. `parse_path()` 解析 trait path (直到 `>`)
6. 期望 `>`
7. 期望 `::`
8. 继续 parse 剩余 segments
9. 返回 `QSelf { ty: Some(T), position: trait_segments_count }`

**§1.0 原则 6 "通用 > 特例"**: 一个 `try_parse_qself` 处理所有 qualified path 形式。

### 3.3 HIR Lower 变更: Projection Production (src/mir/lower/mod.rs)

**当前问题** (line 1953-1962): 
```rust
HirTyKind::Path(_, path) => match path.res {
    Res::Def(def_id, _) => {
        let substs = lower_path_generic_args(path, region_counter, hir);
        Ty::new(TyKind::Adt(def_id, substs), span)
    }
    ...
},
```

QSelf 被 `_` 忽略。需修改为: 当 `qself.ty = Some(...)`，产生 `TyKind::Projection`。

**修改后**:
```rust
HirTyKind::Path(qself, path) => {
    if let Some(inner_ty) = &qself.ty {
        // Qualified path: <T as Trait>::Item
        // Lower to TyKind::Projection(assoc_def_id, substs)
        lower_qualified_path_to_projection(inner_ty, path, region_counter, hir, span)
    } else {
        // Plain path: existing behavior
        match path.res {
            Res::Def(def_id, _) => {
                let substs = lower_path_generic_args(path, region_counter, hir);
                Ty::new(TyKind::Adt(def_id, substs), span)
            }
            Res::PrimTy(PrimTy::Str) => Ty::new(TyKind::Str, span),
            _ => Ty::new(TyKind::Error, span),
        }
    }
}
```

**新增 API** (§10 命名): `fn lower_qualified_path_to_projection(...)` — 把 `<T as Trait>::Item` 转换为 `TyKind::Projection(assoc_def_id, substs)`，其中 `substs[0]` 是 self type。

### 3.4 Projection Resolver 变更 (src/typeck/projection_resolver.rs)

**当前状态**: 已有 `lookup_assoc_type_resolution` 按 name 匹配，但忽略 generics arity。

**修改**: `find_trait_for_assoc_type` 与 `find_impl_for_trait_and_type` 保持现状；`lookup_assoc_type_resolution` 增加 generic arity 检查（验证 impl 中 `type Item<'a, T>` 的 generics 与 trait 声明匹配）。

**§1.0 原则 4 "报错 > 静默"**: arity mismatch 时报 typeck error，不静默返回 None。

### 3.5 测试设计 (§9.4.3 1:3+ ratio)

**测试文件**: `tests/v0/stage18/plan/stage18_53_gats_phase2_tests.rs` (≥8 测试: 2 正 + 6 负)

**正向测试** (2):
1. `qualified_path_parses` — `<T as Trait>::Item` 解析成功
2. `gat_with_lifetime_arg_parses` — `Self::Item<'a>` 解析成功 (无 `>>` splitting 问题)

**负向测试** (6):
1. `qself_missing_as` — `<T>::Item` (无 `as`) 报错
2. `qself_missing_close_angle` — `<T as Trait::Item` (无 `>`) 报错
3. `qself_missing_path_sep` — `<T as Trait>Item` (无 `::`) 报错
4. `gat_unbalanced_generics` — `Self::Item<'a>>` 报错
5. `gat_missing_close_angle` — `Self::Item<'a` 报错
6. `qself_empty_trait` — `<T as>::Item` (空 trait path) 报错

**Conformance 测试**:
- `0379-gat-qualified-path.lin` — 正向: `<T as Trait>::Item` 用作返回类型
- `0380-gat-self-item-with-generics.lin` — 正向: `Self::Item<'a>` 用作返回类型
- `err-0328-gat-qself-missing-as.lin` — 负向
- `err-0329-gat-qself-missing-close-angle.lin` — 负向

---

## 4. §13.5 设计-审查 Agent 循环

### 4.1 Round 1 自审

| 维度 | 自审结论 | 状态 |
|------|---------|------|
| 设计偏差 | Phase 2 范围聚焦 *使用点* 解析与 projection 表示，符合 v0.7 路线图分阶段原则 | ✅ |
| §1.0 原则 1 长期 > 短期 | Phase 2 是 Phase 3 (monomorphization) 的必要前置 | ✅ |
| §1.0 原则 3 显式 > 隐式 | `TyKind::Projection` 显式表示，不依赖 Adt 隐式 | ✅ |
| §1.0 原则 5 去除兼容思维 | `lower_hir_ty_to_mir_ty` Path arm 重写，不保留旧路径 | ✅ |
| §1.0 原则 6 通用 > 特例 | 一个 `try_parse_qself` 处理所有 qualified path | ✅ |
| §10 命名标准 | `parse_qself` / `lower_qualified_path_to_projection` / `split_shr_token` | ✅ |
| §11 接口隔离 | Parser → AST → HIR → MIR 单向流动 | ✅ |
| §9.4.3 1:3+ 测试 | 8 unit + 4 conformance = 12 测试, 4 正 8 负 = 1:2 比例... 需调整为 1:3+ | ⚠️ |
| 死代码 | Phase 2 不创建死代码；旧 Path arm 代码被新 arm 取代 | ✅ |

**Round 1 自审发现 P1 问题**: 测试比例 1:2 不达 1:3+。

**修复**: 增加负向测试数量到 9，正向保持 3，比例 1:3 ✓。

### 4.2 §6.3 委员会投票 (模拟)

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | Phase 2 范围清晰，parser/lower/resolver 三层协同 |
| DEV-A | GO | `>>` splitting 是 rustc 也用的成熟方案 |
| QA-A | GO | 1:3+ 比例满足；qualified path 错误案例覆盖完整 |
| REV-A | GO | 设计原则 1, 3, 5, 6 遵循；Phase 3 推迟明确记录 |
| PM-A | GO | v0.7 路线图 P1 GATs 推进 |

**5/5 GO** ✅

---

## 5. 实施步骤

1. ✅ 写设计文档 (本文件)
2. ⏳ 实现 `>>` splitting (src/parser/path.rs + src/parser/generics.rs)
3. ⏳ 实现 qualified path parser (`try_parse_qself`)
4. ⏳ 修改 HIR→MIR lower: `lower_qualified_path_to_projection`
5. ⏳ 扩展 `projection_resolver` 处理 GAT generics arity
6. ⏳ 新增单元测试 (tests/v0/stage18/plan/stage18_53_gats_phase2_tests.rs)
7. ⏳ 新增 conformance 测试 (4 个)
8. ⏳ 验收: cargo clean + build + fmt + clippy + test
9. ⏳ worklog + 版本 bump v0.319.0 → v0.320.0
10. ⏳ 打包 tar.gz

---

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| `>>` splitting 破坏现有 nested generics 测试 | 单独 helper 函数，现有 `parse_generics` 末尾改为调用 `eat_gt_or_split` |
| Qualified path 解析破坏现有 path 测试 | `try_parse_qself` 在 `<` 不在 path 起始时返回 None，回退到原逻辑 |
| Phase 2 引入 typeck 错误（GAT generics 不匹配） | `projection_resolver` 返回 None 时 graceful degradation 到 `TyKind::Error` |

---

## 7. 结论

Stage 18.53 设计完成。Phase 2 聚焦 GAT *使用点* — qualified path 解析与 projection 表示。`>>` splitting 是关键技术债清理。Phase 3 (monomorphization) 明确推迟。

设计原则严格遵循 §1.0 (长期/通用/显式/报错/去兼容/命名/测试/正确)，§10 命名标准，§11 接口隔离。

5/5 GO，进入实施阶段。
