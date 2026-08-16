# Stage 18.126 — §17 任务规划排版图首次应用 + 结构性技术债扫描

> **Author**: redskaber (PM-A + ARCH-A + REC-A)
> **Date**: 2026-08-16
> **Version**: v0.394.0 (Stage 18.126 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §17 (任务规划排版图) + §13.1 (设计对齐) + §14.5 (深度审查预审)
> **Complexity**: L3 (跨模块扫描 + 9 文件 LOC 违规 + 491 DUMMY + 162 unwrap/expect)
> **Task ID**: stage18.126

---

## 1. 阶段目标

按用户要求严格读取 `docs/stage-committee-process.md` v6.4 §1-§17, 结合审查报告 + stage/plan/gate-review + 复盘 + 开发记录 + 设计文档 + 项目实际, 推进修复并严格 API 命名标准化和接口设计。明确"通解 > 特解、高内聚低耦合、单一职责、避免死代码、避免分散内容"原则。如有简写和缺陷, 记录到开发/设计文档并规划修订计划。

## 2. §13.1 设计对齐结果

| 设计文档 | 设计意图 | 当前实现状态 | 偏差 |
|---------|---------|-------------|------|
| `01-language-specification.md` | Rust-inspired systems language | ✅ v0.1 stable, 6,245 tests | 无偏差 |
| `06-mir.md` | MIR body/place/ty 三层 | ✅ 实现一致 | 无偏差 |
| `07-codegen.md` | LLVM 19 backend, `codegen_crate` 入口 | ✅ 实现一致 | TD-CODEGEN-RESULT: codegen 返回 String 而非 Result |
| `13-stage1-feature-whitelist.md` | v0.1 表面语法白名单 | ✅ 全部实现 | 无偏差 |
| `14-soundness-considerations.md` | typeck/borrowck/mono soundness | ✅ 全部 soundness 项修复 | 无偏差 |
| `16-diagnostics.md` | 8 Kind enums + E001-E900 | ✅ 全部接线 | 无偏差 |

**设计对齐结论**: 6 项核心设计文档全部对齐, 无 B1/B2/B3 偏差, 仅 TD-CODEGEN-RESULT 是 B4 设计灰区（设计文档未要求 CodegenResult, 实现走了 String + panic 捷径, 需补写设计文档）。

## 3. §17 任务规划排版图执行

### 3.1 Step 1: 扫描文档 (§17.2)

扫描结果归档到 `docs/develop/v0/stage-18/stage-18.126-plan-task-layout.md` §1。

**关键发现**:
- 测试: 6,245 (640 lib + 2,663 integration + 2,935 conformance + 7 fuzz), 0 failures
- tech-debt-register: 12 resolved + 15 remaining (v0.2+)
- calibration-data: L2 4-9 轮 / L3 8-15 轮 / 误分类率告警 30%
- 工具链: ⚠️ 当前执行环境缺少 Rust/LLVM 19, 仅能做文档层 + 扫描层工作

### 3.2 Step 2: 依赖图构建 (§17.3)

任务依赖图归档到 plan 文件 §3。本阶段是**单一串行节点流** (S1→S2→S3→S4→S5→S6), 后续 Stage 18.127+ 是 v0.2 P0 mini-cargo 设计阶段。

### 3.3 Step 3-4: 节点流定义 + 递归支持

- **节点 S2**: 9 文件 LOC 阈值违规拆分方案 (9 个子任务 S2.1-S2.9)
- **节点 S3**: Span::DUMMY + unwrap/expect 审计 (5 个子任务 S3.1-S3.5)
- **节点 S4**: tech-debt-register.md 新增 19 项 TD
- **节点 S5**: calibration-data.md 追加 Stage 18.124-18.126 统计
- **节点 S6**: §17 任务规划文档化 (plan 文件最终化)

递归深度 2 层 (S2 → S2.1-S2.9 子任务), 符合 §17.5 递归 ≤3 层要求。

### 3.4 Step 5: 设计-开发-测试节点流

设计节点 (§13.4 J1-J6 + §14.5 D1 预审) → 开发节点 (本阶段仅文档化+扫描) → 测试节点 (5 阶段: 局部/集成/E2E/负向/健壮性)。测试↔设计相互印证: 扫描结果驱动 TD 新增, TD 新增验证扫描完整性。

### 3.5 Step 6: 缺陷纳入

5 项新增结构性 TD (TD-LOC-* × 5) + 8 项 Span::DUMMY 待审计 TD (TD-DUMMY-* × 8) + 6 项 unwrap/expect 静默吞错 TD (TD-UNWRAP-* × 6) = **19 项新增 TD**, 全部规划到 v0.2/v0.3 修复。

### 3.6 Step 7: 优化补充

审查 6 项检查 (任务遗漏/依赖完整性/缺陷纳入/测试覆盖/能力边界/递归合理性), 全部通过。结论: **GO** — 进入 Stage 18.126 执行。

## 4. §14.5 深度审查预审 (D1-D8)

| 维度 | 状态 | 主要发现 |
|------|------|---------|
| D1 架构健康度 | 🟡 | 9 文件超 §13.4 J6 LOC 阈值; projection_resolver 位置错 (TD-PROJECTION-RESOLVER); borrowck 13 unwrap 集中 |
| D2 技术债清单 | ✅ | tech-debt-register.md v0.393.0 完整, 19 项新增结构性 TD |
| D3 测试覆盖深度 | ✅ | 6,245 tests, 正负比例满足 1:3+ (§9.4.3) |
| D4 下一阶段就绪度 | 🟡 | v0.2 P0 mini-cargo 需先解阻 projection_resolver 位置; TD-CODEGEN-RESULT 阻塞 BinaryOp2 修复 |
| D5 设计合理性 | 🟡 | macro_expand.rs 5962 LOC + driver.rs 4018 LOC 是设计债, 应拆分 |
| D6 性能与可扩展性 | ✅ | 无 O(n²) 报告; const_prop 在 loops 中保守 (TD-CONST-PROP-LOOPS) |
| D7 文档与知识传承 | ✅ | lang-design/develop/tests/graph 四层完整; calibration-data.md v0.2 |
| D8 测试路径覆盖 | ✅ | pipeline-test-coverage.md 完整 |

**深度审查结论**: 🟡 **GO-WITH-CONDITIONS** — 文档层 + 扫描层 + 设计层已完成, 代码层修复推迟到具备工具链的执行环境。

## 5. §10 API 命名标准化检查

| 规则 | 状态 | 备注 |
|------|------|------|
| §10.1.1 入口函数 (verb_noun) | ✅ | `codegen_crate`/`tokenize`/`parse_crate`/`resolve_crate`/`check_mir_body_with_tables` 全部合规 |
| §10.1.2 上下文类型 (-Ctxt/-er) | ✅ | `HirLowerCtxt`/`MirLowerCtxt`/`TypeChecker`/`BorrowChecker` |
| §10.1.3 类型前缀 (Hir/Mir/Emit) | ✅ | `HirCrate`/`MirBody`/`EmitType`/`EmitValue` |
| §10.1.4 显式 re-export (无 glob) | ✅ | 全部 mod.rs 已用 explicit list (Stage 3.57 P0-3 fix) |
| §10.1.5 DRY (单一真理源) | ✅ | `DefKind`/`BorrowKind` 跨阶段 re-export 合规 |
| §10.1.6 deprecated note | ✅ | Stage 3.63 已全量标记 |
| §10.1.7 函数命名前缀 | ✅ | `lex_`/`parse_`/`lower_`/`resolve_`/`check_`/`emit_`/`codegen_` 全部合规 |

**结论**: API 命名标准化 100% 合规, 无新增 L-NAMING-N 债务。

## 6. §11 接口隔离检查

| 检查项 | 状态 | 备注 |
|--------|------|------|
| codegen 不调用 mir::lower | ✅ | grep 零匹配 |
| codegen 不调用 typeck | ✅ | grep 零匹配 |
| codegen 不调用 driver | ✅ | grep 零匹配 |
| typeck 不直接读 HIR | ⚠️ | `projection_resolver.rs` 位置错 (TD-PROJECTION-RESOLVER, v0.2 Phase 2 修复) |
| driver 是唯一 HIR 读者 | ✅ | 只有 driver.rs 直接构造/读取 HIR |
| 元数据预计算 | ✅ | body_metas/fn_name_by_def_id/FieldTyTable 均预计算 |
| 无 glob exports | ✅ | 0 violations |
| 错误路径覆盖 | ⚠️ | BinaryOp2 用 panic!() 而非 CodegenError (TD-CODEGEN-RESULT, v0.2 Phase 2) |

**结论**: 1 项 open L-PIPE-N (TD-PROJECTION-RESOLVER) + 1 项 error-path 偏差 (TD-CODEGEN-RESULT), 全部已规划修复。

## 7. §2.2 设计原则合规

| 原则 | 状态 | 备注 |
|------|------|------|
| 1. 长期 > 短期 | ✅ | 19 项 TD 全部规划长期修复 (v0.2/v0.3), 不打补丁 |
| 2. 整体 > 局部 | ✅ | 9 文件 LOC 拆分方案从整体架构出发, 非按 LOC 切片 |
| 3. 显式 > 隐式 | ✅ | TD-UNWRAP-* 修复方向: unwrap → expect("message") 显式化 |
| 4. 报错 > 静默 | 🟡 | 162 unwrap/expect 静默吞错, 违反此原则; 已规划修复 |
| 5. 去除兼容思维 | ✅ | v0.1 不向后兼容, 旧代码直接替换 |
| 6. 通用 > 特例 | ✅ | TD-CODEGEN-RESULT 修复方向: 通用 CodegenResult 而非 BinaryOp2 特例 panic |
| 7. API 命名标准化 | ✅ | 见 §5 |
| 8. 设计驱动测试, 测试验证设计 | ✅ | 6,245 tests 覆盖全部设计文档场景 |
| 9. 正确 > 妥协 | 🟡 | BinaryOp2 panic 是妥协 (Stage 18.119), 已记录待修复 |

## 8. 简化与缺陷记录 (per 用户要求)

### 8.1 已记录的简化/缺陷 (来自 tech-debt-register.md)

| ID | 简化/缺陷描述 | 原因 | 修订计划 |
|----|-------------|------|---------|
| TD-CODEGEN-RESULT | codegen 返回 String 而非 Result, BinaryOp2 panic | Stage 18.99-18.103 单态化期省事 | v0.2 Phase 2: 改 CodegenResult + `?` 传播 |
| TD-PROJECTION-RESOLVER | projection_resolver 在 typeck/ 下, 应在 driver | Stage 18.87 GATs Phase 3 临时位置 | v0.2 Phase 2: 移到 driver::post_typeck |
| TD-INT-UINT-VAR | unify table Int/Uint 合并 | 早期开发期省事 | v0.2 Phase 2: 分离 IntOrUintVar |
| TD-BINARYOP2-PANIC | BinaryOp2 panic 替代报错 | Stage 18.119 临时修复 | v0.2 Phase 2: 依赖 TD-CODEGEN-RESULT |
| TD-LOC-MACRO-EXPAND | macro_expand.rs 5962 LOC 单一职责违反 | macro_rules! 全功能集中 | v0.2 P2: 按 hygiene/repetition/fragment 三层拆分 |
| TD-LOC-DRIVER | driver.rs 4018 LOC 单一职责违反 | 编排层全功能集中 | v0.2 P2: 按 4 层拆分 |
| TD-LOC-MIR-LOWER-EXPR | mir/lower/expr_operand.rs 3596 LOC | 表达式 lowering 全集中 | v0.2 P2: 按 5 类拆分 |
| TD-LOC-MIR-LOWER-MOD | mir/lower/mod.rs 2857 LOC | body lowering + local decls 混合 | v0.2 P2: 拆分 |
| TD-LOC-TYPECK-CHECKER | typeck/checker.rs 2635 LOC | typeck 主入口全集中 | v0.2 P2: 按 unify/infer/coerce/check 拆分 |
| TD-DUMMY-* (8 项) | 491 Span::DUMMY 未做 A/B 分类 | Stage 18.115-18.117 仅治理 4 个文件 | v0.2 P2: 逐个审计 |
| TD-UNWRAP-* (6 项) | 162 unwrap/expect 静默吞错 | 早期开发期省事 | v0.2 P2: 改 expect("...") 或 `?` 传播 |

### 8.2 新增简化/缺陷修订计划 (本阶段产出)

**Stage 18.127 (待工具链就绪)**:
1. 执行 9 文件 LOC 拆分方案 (TD-LOC-* × 5 优先, 其余 4 项 v0.3)
2. 执行 Span::DUMMY 审计 (TD-DUMMY-* × 8)
3. 执行 unwrap/expect 治理 (TD-UNWRAP-* × 6)

**Stage 18.128+ (v0.2 P0)**:
1. mini-cargo 项目系统 (TD-SINGLE-FILE)
2. CodegenResult 传播 (TD-CODEGEN-RESULT, 解阻 TD-BINARYOP2-PANIC)
3. projection_resolver 位置修复 (TD-PROJECTION-RESOLVER)

## 9. 验收 (§3.2)

> **关键限制**: 当前执行环境缺少 Rust/LLVM 19 工具链, 无法执行 cargo check/test/fmt/clippy。本阶段仅完成文档层 + 扫描层 + 设计层工作。

**已完成的验收**:
- ✅ §17 任务规划排版图 plan 文件产出 (`docs/develop/v0/stage-18/stage-18.126-plan-task-layout.md`)
- ✅ tech-debt-register.md 新增 19 项 TD + 分类索引 (§6.2.1 强制结构)
- ✅ calibration-data.md 追加 Stage 18.124-18.126 统计
- ✅ v0.1-capability-boundaries.md 版本同步
- ✅ §10 API 命名 100% 合规
- ✅ §11 接口隔离 1 项 open (TD-PROJECTION-RESOLVER, 已规划)
- ✅ §2.2 设计原则 9/9 评估 (7 ✅ + 2 🟡)

**未完成的验收 (推迟到具备工具链的执行环境)**:
- ❌ cargo check --features llvm-backend (需 Rust + LLVM 19)
- ❌ cargo test --features llvm-backend (需 Rust + LLVM 19)
- ❌ cargo fmt --check (需 rustfmt)
- ❌ cargo clippy --all-targets (需 clippy)

## 10. 文档同步 (§8)

| 文档 | 路径 | 更新内容 |
|------|------|---------|
| plan 文件 | `docs/develop/v0/stage-18/stage-18.126-plan-task-layout.md` | 新建 (§17 任务规划排版图) |
| dev-log | `docs/develop/v0/stage-18/stage-18.126-dev-log.md` | 新建 (本文件) |
| 技术债登记册 | `docs/develop/v0/tech-debt-register.md` | v0.387.0 → v0.393.0 + 19 项新增 TD + §4 分类索引 |
| 流程校准数据池 | `docs/develop/v0/calibration-data.md` | v0.1 → v0.2 + Stage 18.125-18.126 统计 + §3.5 教训归档 |
| 能力边界 | `docs/develop/v0/v0.1-capability-boundaries.md` | v0.388.0 → v0.393.0 |
| worklog | `docs/worklog.md` | 追加 Stage 18.126 entry |
| Cargo.toml | `Cargo.toml` | v0.393.0 → v0.394.0 |
| README.md | `README.md` | v0.393.0 → v0.394.0 |

## 11. Stage Summary

- **Stage 18.126 PASSED** — §17 任务规划排版图首次实际应用 + 结构性技术债扫描
- **复杂度**: L3 (跨模块扫描 + 9 文件 LOC 违规 + 491 DUMMY + 162 unwrap/expect)
- **新增 19 项结构性 TD**: TD-LOC-* × 5 + TD-DUMMY-* × 8 + TD-UNWRAP-* × 6, 全部规划到 v0.2/v0.3 修复
- **API 命名**: 100% 合规 (§10.1.1-§10.1.7 全部 ✅)
- **接口隔离**: 1 项 open (TD-PROJECTION-RESOLVER, v0.2 Phase 2 修复)
- **设计原则**: 9/9 评估 (7 ✅ + 2 🟡, 已规划修复)
- **工具链限制**: 当前执行环境缺少 Rust/LLVM 19, 仅完成文档层 + 扫描层 + 设计层; 代码层修复推迟到 Stage 18.127+
- **v0.394.0**: doc-fix bump (§17 任务规划 + 结构性技术债扫描)
- **下一步**: Stage 18.127 — 待工具链就绪后执行 9 文件 LOC 拆分 + Span::DUMMY 审计 + unwrap/expect 治理
