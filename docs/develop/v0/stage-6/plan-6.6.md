# Stage 6.6 开发计划(回填):mir/lower/mod.rs 拆分 — control_flow 提取(TD-011 第六步) — 🎉 mod.rs < 2000 LOC!

> **状态**: ✅ Complete (本 plan 于 Stage 12.9 从 `gate-review-6.6.md` 回填重建)
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2(原始执行);v3.21 §14.4 重构即架构设计判据(回填适用)
> **原始执行**: Stage 6.6(TD-011 step 6) — 当時未产出独立 plan 文档,仅 gate review
> **回填日期**: 2026-07-26(Stage 12.9 backfill per r217 stages-5-8 audit §7 P2 item 6)

## 1. 目标(reconstructed from gate-review-6.6.md)

继续偿还 TD-011。第六步:将 control flow(控制流)相关函数从
`mir/lower/mod.rs` 提取到独立模块 `mir/lower/control_flow.rs`。

本步骤达成 🎉 关键里程碑:**`mir/lower/mod.rs` 从 2452 LOC 降至 1980 LOC,
首次跌破 2000 LOC 阈值**(累计 6 步 TD-011 拆分共减少 1366 LOC,mod.rs 从
原始 3346 LOC 降至 1980 LOC,-40.8%)。

依据 §14.4 "重构即架构设计",本步骤的架构判据聚焦于 J1(架构对齐)/
J6(科学粒度):control flow 是 MIR lowering 中体量最大的子模块之一(if/match/
loop/while/for/break/continue 的 lower 逻辑集中),提取后既达成 mod.rs < 2000 LOC
里程碑,又为 TD-011 step 7(6.10 expr_operand 拆分,后续)留出操作空间。

## 2. MUV 拆分(reconstructed from gate-review-6.6.md "拆分结果" 表)

### 2.1 提取的函数

| 函数组 | LOC(估) | 职责 |
|--------|----------|------|
| `lower_if` / `lower_match` 系列 | ~200 | if/match 表达式 → MIR basic blocks + SwitchInt + Goto |
| `lower_loop` / `lower_while` / `lower_for` 系列 | ~180 | 循环结构 → MIR basic blocks + Loop + Break/Continue |
| `lower_block` + 早返回 / divergence 处理 | ~60 | HirBlock → MIR statement sequence + divergence 标记 |
| 相关 use / 辅助函数 | ~22 | MirLowerCxt 引用、BasicBlock 分配、Terminator 构造 |

(精确的逐函数清单在原始执行时未单独记录,本表由 gate-review-6.6.md
"mir/lower/control_flow.rs — 462 LOC 新建"反推;462 LOC 是 TD-011 7 步中
单步提取量最大的步骤,反映 control flow 在 MIR lowering 中的核心地位。)

### 2.2 §16 接口隔离

提取后 `mir/lower/control_flow.rs` 依赖:
- `mir::body::*`(`MirBody`, `BasicBlock`, `StatementKind`, `TerminatorKind`, `SwitchTargets`)
- `mir::place::*`(`Place`, `Local`, `Operand`, `Rvalue`)
- `mir::lower::*`(`MirLowerCtxt`, basic block 分配 API)
- `mir::lower::pattern_bindings::*`(从 6.3 提取的模块,match arm lowering 调用)
- `hir::*`(`HirExpr`, `HirExprKind::If/Match/Loop/While/For`, `HirBlock`, `HirStmt`, `Pat`)

所有依赖单向(`mir::lower` 内部模块间,control_flow → pattern_bindings),无循环。✅

### 2.3 命名标准化

无新公共 API — 仅内部模块重组。相关函数从 `fn` 改为 `pub(crate) fn`。

## 3. §14.4 J1-J6 判据(reconstructed)

| 判据 | 评估 | GO/NO-GO |
|------|------|----------|
| J1 架构对齐 | control flow 是 MIR lowering 的核心子模块,与 field_resolution(6.5)/ expr_operand(6.10)正交 | ✅ GO |
| J2 单一职责 | 新模块仅负责控制流结构(if/match/loop/while/for)的 MIR lowering | ✅ GO |
| J3 单向流动 | 依赖 `pattern_bindings`(6.3)辅助 match arm lowering,产出 MIR `BasicBlock` + `Terminator`;无反向依赖 | ✅ GO |
| J4 编译表达完整 | 拆分后所有 rust types 仍可被现有 MIR 表达,无新 type 引入 | ✅ GO |
| J5 阶段划分 | 作为 TD-011 step 6,在 6.5(已完成)与 6.7-6.10(后续 codegen 拆分 + expr_operand 拆分)之间 | ✅ GO |
| J6 科学粒度 | 472 LOC 提取量是 TD-011 单步最大,与 control flow 在 lowering 中的核心地位相称;达成 mod.rs < 2000 LOC 里程碑 | ✅ GO |

**6/6 GO**(回填判据,原始执行时 v3.20 未要求 J1-J6 表格,但拆分实际满足全部判据)。

## 4. 验收标准(reconstructed)

1. `cargo test`:**1881 passed, 0 failed, 2 ignored**(行为等价重构,0 新测试)
2. `cargo fmt --check`:clean(exit 0)
3. `cargo clippy --all-targets`:0 warnings, 0 errors
4. `mir/lower/mod.rs` LOC 减少 ≥ 400(实际 -472)
5. 新建 `mir/lower/control_flow.rs` 模块,LOC ≤ 500(实际 462)
6. §16 依赖方向单向,control_flow → pattern_bindings,无循环
7. 🎉 **里程碑:`mir/lower/mod.rs` LOC < 2000**(实际 1980)

## 5. 实际执行结果(from gate-review-6.6.md)

**CI/CD**(原始执行,2026-07-24):

```
cargo clean: clean (569.6 MiB removed)
cargo test: 1881 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

**拆分结果**:

| 文件 | Before | After | 变化 |
|------|--------|-------|------|
| `mir/lower/mod.rs` | 2452 LOC | 1980 LOC | -472 LOC (-19.2%) |
| `mir/lower/control_flow.rs` | — | 462 LOC | 新建 |

**🎉 TD-011 milestone: `mir/lower/mod.rs` below 2000 LOC!**

**TD-011 累计进度**(截至 6.6 完成):

| Split | Module | LOC extracted | mod.rs after |
|-------|--------|--------------|--------------|
| 6.1 | adt_layout.rs | 153 | 3193 |
| 6.2 | closure_capture.rs | 158 | 3035 |
| 6.3 | pattern_bindings.rs | 305 | 2730 |
| 6.4 | overflow_assert.rs | 74 | 2656 |
| 6.5 | field_resolution.rs | 204 | 2452 |
| 6.6 | control_flow.rs | 472 | 1980 |
| **Total** | **6 modules** | **1366 LOC** | **1980 (was 3346, -40.8%)** |

**Gate review verdict**: 5/5 GO → PASS

## 6. 关联文档

- `docs/develop/v0/stage-6/gate-review-6.6.md` — 原始 gate review(本 plan 的回填来源,含 mod.rs < 2000 LOC 里程碑记录)
- `docs/develop/v0/stage-6/plan-6.1.md` ~ `plan-6.5.md` — 前序 plan(TD-011 step 1-5,参考格式;6.4/6.5 同为本 Stage 12.9 回填)
- `docs/develop/v0/stage-6/plan-6.10.md` — 后续 plan(TD-011 step 7:expr_operand 拆分,该步骤当時有独立 plan)
- `docs/develop/v0/stage-12/cross-stage-audit-r217-stages-5-8.md` §3 + §7 P2 item 6 — 回填来源审计
- `docs/stage-committee-process.md` §14.4(重构即架构设计,J1-J6 判据)+ §17.3 时期 2(原始执行的流程版本)
- `docs/develop/v0/stage-6/README.md` — Stage 6 README(本 plan 使 Stage 6 plan 文件数从 15 → 16;待 6.4/6.5/6.6 全部回填后达 18,与 gate-review 文件数对齐)

---

**回填日期**: 2026-07-26 (Stage 12.9)
**原始执行日期**: 2026-07-24 (Stage 6.6, v0.12.4 → v0.12.5)
