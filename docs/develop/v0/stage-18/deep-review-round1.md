# Stage 18 Deep Review Report (Round 1) — v0.366.0

> **Review date**: 2026-08-11
> **Reviewer**: Super Z (main) — ARCH-A + QA-A + REV-A roles
> **Baseline version**: v0.366.0 (Stage 18.98)
> **Test count**: 643 lib + 2,775 integration + 2,935 conformance + 7 fuzz = 6,360 total
> **Process**: stage-committee-process.md v5.0 §14.5 (D1-D8) + §14.8 (design writeback)

## 1. 执行摘要

当前阶段（Stage 18.98 后）编译管道**架构健康**，可以进入 v0.2 下一阶段
（mini-cargo / 完整单态化）。但发现 **3 项 P1 阻塞项**必须在进入 v0.2 前修复：

- **P1-SOUND**: `typeck/unify.rs:674-683` FnDef↔FnPtr 统一未检查签名（soundness 漏洞）
- **P1-DOC**: `docs/tests/matrix.md` + `pipeline-test-coverage.md` 版本/计数过时（Stage 18.97 声称同步但未实际更新版本字段）
- **P1-DESIGN**: `docs/lang-design/06-mir.md` 未记录 Stage 18.96 MIR opt 接线

另有 **8 项 P2** + **5 项 P3** 技术债，均有明确偿还计划。

**建议行动**: **GO-WITH-CONDITIONS** — 完成 3 项 P1 后进入 v0.2。

## 2. 八维度审查结论

### D1. 架构健康度

**现状**: 架构基本健康，§11 接口隔离大部分遵守。

- ✅ 0 个 data-flow back-edit（下游不修改上游数据结构）
- ✅ codegen 不调用 `hir::lower::` 或 `mir::lower::` 内部函数
- ✅ 0 个 `pub use X::*` glob re-export（§10.1 合规）
- ✅ 556 个 `pub fn` 全部遵循 §23 `<verb>_<noun>` 命名
- ⚠️ **1 个跨阶段耦合违规**: `typeck/projection_resolver.rs:180,227` 调用 `mir::lower::lower_hir_ty_to_mir_ty_with_hir`（应为 MIR data-sink 模式）
- ⚠️ **1 个 dead-code 模块**: `borrowck/region_inference.rs` 1776 行中 ~1300 行为 v0.2 基础设施（已 `#[allow(dead_code)]`，但存在 bit-rot 风险）
- ⚠️ **1 个零生产调用模块**: `codegen/dyn_trait_emit.rs` 294 行仅测试调用

**风险**: `projection_resolver` 耦合会阻碍 v0.2 单态化重构（单态化需要 MIR lower 可独立重跑）。

**建议**: 
1. P2: 重构 `projection_resolver` 使用 MIR data-sink（类似 `AggregateKind::Adt` 的 `field_tys` 模式）
2. P3: 将 `dyn_trait_emit.rs` 移至 `tests/common/`

### D2. 技术债清单

| ID | 描述 | 优先级 | 偿还计划 |
|----|------|--------|---------|
| TD-13 | `unify.rs:674-683` FnDef↔FnPtr 未检查签名（soundness） | **P1** | Stage 18.99（本阶段修复） |
| TD-DUP1 | `types_match_loose` + `can_coerce` 逻辑重复 ~190 行 | P2 | v0.2 引入 `TypeRelation` trait |
| TD-DUP2 | `format_ty` 在 3 处重复定义 | P2 | v0.2 提取到 `mir::ty` |
| TD-DUP3 | `infer_place` (typeck) + `place_ty` (borrowck) 重复 | P2 | v0.2 提取到 `mir::place` |
| TD-SPAN | 1331 个 `Span::DUMMY`，其中 ~180 个应传播真实 span | P2 | v0.2 MIR lower span 传播 |
| TD-UNWRAP1 | `resolve/module_build.rs:427` `.unwrap()` 无守卫 | P2 | v0.2 改 `if let Some` |
| TD-UNWRAP2 | `codegen/llvm/helpers.rs:41` `CString::new(s).unwrap()` | P2 | v0.2 用 `cstr_or_err` |
| TD-1 | `codegen/rvalue.rs:523` BinaryOp2 fallback 返回 "0" | P2 | v0.2 CodegenResult |
| TD-6 | `driver.rs:1505` struct 全 Copy 字段不自动 Copy | P2 | v0.2 field-level Copy |
| TD-9 | `checker.rs:1074` Deref on non-Ref 模式绑定 | P2 | v0.2 引用类型跟踪 |
| TD-11 | `checker.rs:1603` Int↔Uint 同宽 loose match（workaround） | P2 | v0.2 IntOrUintVar |
| TD-15 | `expr_operand.rs:1516` 闭包 capture 总 Copy | P2 | Stage 13.5+ |
| TD-16 | `expr_operand.rs:1563` `move` 关键字 no-op | P2 | Stage 13.5+ |

**13/27 技术债标记指向 v0.2** — 建议组建 v0.2-prep epic 统一 triage。

### D3. 测试覆盖深度

**现状**:
- 实际总数: **6,360** (lib 643 + integration 2,775 + conformance 2,935 + fuzz 7)
- `matrix.md` 声称 6,195 — **过时 165 个**（Stage 18.97/18.98 新增未同步）
- 正负比例: 文件级满足 1:3+（`negative_cases_tests.rs` 1:8.4，`typeck_tests.rs` 1:14.5），但 `integration_tests.rs` (1.18:1) 和 `codegen_tests.rs` (0.88:1) 偏正
- Fuzz: 7 个 stress 测试（无 panic 不变式），无 proptest

**风险**:
- **Stage 18.98 嵌套 Adt 递归分支零测试**: `Vec<Vec<i32>>` vs `Vec<Vec<bool>>` 未覆盖 `types_match_loose` 递归调用
- **35 个 runtime tests OOM-skipped**: 在 4GB RAM 开发机上被跳过，隐藏回归风险

**建议**:
1. **P1**: 立即添加嵌套 Adt 递归测试（Stage 18.99）
2. P2: 将 runtime tests 拆分为更小二进制避免 OOM
3. P2: 引入 proptest 做属性测试

### D4. 下一阶段就绪度

| 下一阶段需求 | 当前状态 | 差距 | 解阻计划 |
|-------------|---------|------|---------|
| 单态化 infra (collect_mono_items) | ✅ | — | — |
| Substs 传播 (TyKind::Adt) | ✅ Stage 16.52 | — | — |
| Adt substs soundness | ✅ Stage 18.98 | — | — |
| FnDef↔FnPtr soundness | ❌ TD-13 | unify 未检查签名 | Stage 18.99 修复 |
| GAT Phase 4 (projection 具体化) | ⚠️ | projection_resolver 耦合 | v0.2 重构 |
| 项目系统 (mini-cargo) | ❌ | 无 crate 图 | v0.2 新建 |
| 完整 stdlib | ❌ | 仅 facade | v0.2 P1 |

**结论**: 修复 TD-13 后，单态化方向可推进；mini-cargo 方向无阻塞。

### D5. 设计合理性

**过度设计**:
- `region_inference.rs` 1776 行中 ~1300 行 v0.2 基础设施（已标注，可接受）
- `codegen/dyn_trait_emit.rs` 294 行零生产调用（应移至 tests/）

**设计不足**:
- `codegen/rvalue.rs` BinaryOp2 fallback 返回 "0" + eprintln（非 CodegenResult）
- `typeck/unify.rs` 无 HKT（v0.1 能力边界，可接受）

**API 一致性**: ✅ 全部合规（0 glob，556 pub fn 命名标准）

### D6. 性能与可扩展性

**现状**:
- ✅ 核心管道（typeck/borrowck/lower/resolve）无 O(n²) 热点
- ⚠️ `driver.rs:1298,1347,1370` `fn_sig_table.sigs.clone()` 每函数克隆完整签名表 — **O(F×S)**
- ⚠️ 461 个 `.clone()`，多数为 Ty（Arc-cheap），但 sigs clone 是真热点

**建议**:
- P2: 将 `TypeChecker::fn_sigs` 从 owned 改为 `&FnSigTable` 引用

### D7. 文档与知识传承

**现状**:
- ✅ 99.3% 模块有 `//!` 文档（仅 `src/bin/main.rs` 缺失）
- ❌ `docs/tests/matrix.md` 版本/计数过时（v0.364.0 → 应 v0.366.0，计数差 165）
- ❌ `docs/tests/pipeline-test-coverage.md` 体内容停留在 Stage 14.x（计数 5171/7122）
- ❌ `docs/lang-design/06-mir.md` 未记录 Stage 18.96 MIR opt 接线
- ❌ `docs/lang-design/04-ownership-borrowing.md` 未反映 Stage 15.x NLL
- ⚠️ 35 个 stage-18.* 子阶段缺 design doc（18.32-18.44, 18.60-18.70 等）

**建议**:
1. **P1**: 同步 `matrix.md` + `pipeline-test-coverage.md` 版本/计数
2. **P1**: `06-mir.md` 添加 §9.3 MIR opt 接线说明
3. P2: `04-ownership-borrowing.md` 更新 NLL 实现状态
4. P3: 为缺失的 35 个 stage-18 子阶段添加一行 "merged into X.Y" 占位

### D8. 测试路径覆盖与流水线印证

**现状**:
- ✅ 流水线图准确（含 macro_expand + writeback + MIR opt 13 个阶段）
- ⚠️ Tier 2 矩阵未反映 Stage 18.96 后扩展的 3 个新过渡：
  - Borrowck→Writeback（缺独立测试）
  - Writeback→MIR opt（缺独立测试）
  - MIR opt→Codegen（缺独立测试）
- ✅ E2E: 35 个 runtime tests（但 OOM-skipped）

**建议**:
- P2: 为 3 个新过渡添加独立集成测试

## 3. 委员会投票

| 角色 | 投票 | 理由 |
|------|------|------|
| ARCH-A | GO-WITH-CONDITIONS | 架构健康，但 TD-13 soundness 必须修复 |
| DEV-A | GO-WITH-CONDITIONS | 代码质量高，但 docs 同步滞后 |
| QA-A | GO-WITH-CONDITIONS | 测试覆盖强，但嵌套 Adt 分支无测试 |
| ALG-C | GO | 算法无 O(n²)，sig clone 可优化 |
| SKL-A | GO-WITH-CONDITIONS | 技能传承需补 docs |

## 4. 行动计划

### 本阶段追加任务（Stage 18.99 — Deep Review Fixes）

| ID | 任务 | 优先级 |
|----|------|--------|
| 18.99.1 | 修复 TD-13: FnDef↔FnPtr unify 检查签名 | P1 |
| 18.99.2 | 添加嵌套 Adt 递归测试 (Vec<Vec<i32>> vs Vec<Vec<bool>>) | P1 |
| 18.99.3 | 同步 matrix.md + pipeline-test-coverage.md 版本/计数 | P1 |
| 18.99.4 | 06-mir.md 添加 §9.3 MIR opt 接线说明 | P1 |
| 18.99.5 | 输出本 deep-review 报告 | ✅ 本文件 |

### 下一阶段优先任务（v0.2）

1. mini-cargo 项目系统（4-6 stages）
2. 完整单态化 GAT Phase 4（6-8 stages，依赖 projection_resolver 重构）
3. 完整 stdlib（8-12 stages）

### 技术债偿还顺序

1. P1: TD-13 (Stage 18.99)
2. P2: TD-DUP1/2/3, TD-SPAN, TD-UNWRAP1/2, TD-1/6/9/11 (v0.2 batch)
3. P3: dyn_trait_emit 迁移, region_inference v0_2 feature gate, 缺失 stage doc (v0.3)

## 5. 结论

**GO-WITH-CONDITIONS** — 完成 Stage 18.99 的 4 项 P1 修复后，进入 v0.2。

## 6. 设计偏差清单（§14.8）

| 设计文档章节 | 偏差类型 | 偏差描述 | 最优判断 | 重构判断 | 回写动作 |
|-------------|---------|---------|---------|---------|---------|
| `06-mir.md` §9.3 | B2 (实现超前设计) | 设计文档未记录 Stage 18.96 MIR opt 接线 | 实现正确 | 否 | Stage 18.99 补写 |
| `04-ownership-borrowing.md` §4.6 | B3 (设计超前实现) | RegionInferenceContext 仅 4/10+ API 生产调用 | 实现保守 | 否 | v0.2 补完或标注 |
| `types_match_loose` / `can_coerce` | B4 (重复设计) | 两函数逻辑重复 ~190 行 | 应统一 | v0.2 | v0.2 引入 TypeRelation |
| `projection_resolver` §11 | B1 (实现偏离设计) | 调用 mir::lower 内部函数 | 应 data-sink | v0.2 | v0.2 重构 |
